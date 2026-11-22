use std::io::{Read, Write};
#[cfg(unix)]
use std::os::unix::process::CommandExt;
use std::process::{Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::thread;
use std::time::Duration;

#[cfg(unix)]
fn kill_group(pid: u32) {
    if pid > 1 && pid <= i32::MAX as u32 {
        let _ = unsafe { libc::kill(-(pid as i32), libc::SIGKILL) };
    }
}

#[cfg(not(unix))]
fn kill_group(_pid: u32) {}

pub const OUTPUT_CAP: usize = 1 << 20;
pub const BUILD_TIMEOUT: Duration = Duration::from_secs(120);
pub const PROBE_TIMEOUT: Duration = Duration::from_secs(5);
pub const HARNESS_TIMEOUT: Duration = Duration::from_secs(10);
const DRAIN_TIMEOUT: Duration = Duration::from_secs(2);
const ALLOWED_ENV: [&str; 5] = ["PATH", "HOME", "CARGO_HOME", "RUSTUP_HOME", "TMPDIR"];

pub struct Captured {
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub truncated: bool,
    pub timed_out: bool,
    pub success: bool,
    pub status: ExitStatus,
    pub write_error: Option<String>,
}
fn sanitized_with(
    program: impl AsRef<std::ffi::OsStr>,
    lookup: impl Fn(&str) -> Option<std::ffi::OsString>,
) -> Command {
    let mut command = Command::new(program);
    command.env_clear();
    for key in ALLOWED_ENV {
        if let Some(value) = lookup(key) {
            command.env(key, value);
        }
    }
    command
}

pub fn sanitized(program: impl AsRef<std::ffi::OsStr>) -> Command {
    sanitized_with(program, |key| std::env::var_os(key))
}

fn drain(mut reader: impl Read, detach: Arc<AtomicBool>) -> (Vec<u8>, bool) {
    let mut bytes = Vec::new();
    let mut truncated = false;
    let mut buffer = [0u8; 8192];
    loop {
        if detach.load(Ordering::Relaxed) {
            break;
        }
        match reader.read(&mut buffer) {
            Ok(0) => break,
            Ok(count) => {
                let remaining = OUTPUT_CAP.saturating_sub(bytes.len());
                if count > remaining {
                    truncated = true;
                    bytes.extend_from_slice(&buffer[..remaining]);
                } else {
                    bytes.extend_from_slice(&buffer[..count]);
                }
            }
            Err(_) => break,
        }
    }
    (bytes, truncated)
}

pub fn run(
    command: &mut Command,
    payload: Option<&[u8]>,
    timeout: Duration,
) -> std::io::Result<Captured> {
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    if payload.is_some() {
        command.stdin(Stdio::piped());
    }
    #[cfg(unix)]
    command.process_group(0);
    let mut child = command.spawn()?;
    let stdout = child.stdout.take().expect("stdout pipe");
    let stderr = child.stderr.take().expect("stderr pipe");
    let detach = Arc::new(AtomicBool::new(false));
    let (out_sender, out_receiver) = mpsc::channel();
    let (err_sender, err_receiver) = mpsc::channel();
    let out_detach = detach.clone();
    let err_detach = detach.clone();
    thread::spawn(move || {
        let _ = out_sender.send(drain(stdout, out_detach));
    });
    thread::spawn(move || {
        let _ = err_sender.send(drain(stderr, err_detach));
    });

    let mut write_error = None;
    if let Some(bytes) = payload {
        let mut stdin = child.stdin.take().expect("stdin pipe");
        if let Err(error) = stdin.write_all(bytes) {
            write_error = Some(error.to_string());
        }
        drop(stdin);
    }

    let take =
        |receiver: mpsc::Receiver<(Vec<u8>, bool)>| match receiver.recv_timeout(DRAIN_TIMEOUT) {
            Ok(result) => result,
            Err(_) => {
                detach.store(true, Ordering::Relaxed);
                (Vec::new(), true)
            }
        };
    let deadline = std::time::Instant::now() + timeout;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => {}
            Err(error) => {
                kill_group(child.id());
                let _ = child.kill();
                let _ = child.wait();
                let _ = take(out_receiver);
                let _ = take(err_receiver);
                return Err(error);
            }
        }
        if std::time::Instant::now() >= deadline {
            kill_group(child.id());
            let _ = child.kill();
            let status = child.wait()?;
            let (stdout, out_truncated) = take(out_receiver);
            let (stderr, err_truncated) = take(err_receiver);
            return Ok(Captured {
                stdout,
                stderr,
                truncated: out_truncated || err_truncated,
                timed_out: true,
                success: false,
                status,
                write_error,
            });
        }
        thread::sleep(Duration::from_millis(2));
    };

    let (stdout, out_truncated) = take(out_receiver);
    let (stderr, err_truncated) = take(err_receiver);
    Ok(Captured {
        stdout,
        stderr,
        truncated: out_truncated || err_truncated,
        timed_out: false,
        success: status.success(),
        status,
        write_error,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::ffi::{OsStr, OsString};

    #[test]
    fn sanitized_command_drops_inherited_build_variables() {
        let variables = [
            ("PATH", "/usr/bin:/bin"),
            ("C2TV_TEST_INHERITED", "leak"),
            ("RUSTFLAGS", "-D warnings"),
        ];
        let command = sanitized_with("sh", |key| {
            variables
                .iter()
                .find(|(name, _)| *name == key)
                .map(|(_, value)| OsString::from(*value))
        });
        let envs: Vec<_> = command.get_envs().collect();
        assert!(envs
            .iter()
            .any(|(key, value)| { *key == "PATH" && *value == Some(OsStr::new("/usr/bin:/bin")) }));
        assert!(envs
            .iter()
            .all(|(key, value)| ALLOWED_ENV.contains(&key.to_str().unwrap()) && value.is_some()));
    }

    #[test]
    fn output_beyond_budget_is_flagged() {
        let mut command = sanitized("sh");
        command.args(["-c", "head -c 2000000 /dev/zero"]);
        let captured = run(&mut command, None, BUILD_TIMEOUT).unwrap();
        assert!(captured.success);
        assert!(captured.truncated);
        assert_eq!(captured.stdout.len(), OUTPUT_CAP);
    }

    #[test]
    fn timed_out_command_is_killed_and_reaped() {
        let mut command = sanitized("sh");
        command.args(["-c", "sleep 30"]);
        let captured = run(&mut command, None, Duration::from_millis(50)).unwrap();
        assert!(captured.timed_out);
        assert!(!captured.success);
    }
    #[cfg(unix)]
    #[test]
    fn timeout_kills_grandchildren() {
        use std::fs;
        use std::thread::sleep;
        let ps = Command::new("ps")
            .args(["-p", "1", "-o", "stat="])
            .output()
            .expect("ps is required to verify process-group kill; install procps");
        assert!(
            ps.status.success(),
            "ps -p 1 failed: {}",
            String::from_utf8_lossy(&ps.stderr)
        );
        let sleeper = Command::new("sleep")
            .arg("0")
            .status()
            .expect("sleep is required to spawn the grandchild");
        assert!(sleeper.success(), "sleep 0 failed");
        let pid_file =
            std::env::temp_dir().join(format!("c2tv-grandchild-{}.pid", std::process::id()));
        let cleanup = || {
            let _ = fs::remove_file(&pid_file);
        };
        cleanup();
        let script = format!("sleep 30 & echo $! > {}; wait", pid_file.display());
        let mut command = sanitized("sh");
        command.args(["-c", &script]);
        let captured = run(&mut command, None, Duration::from_secs(2)).unwrap();
        assert!(captured.timed_out);
        let mut pid = String::new();
        for _ in 0..100 {
            if let Ok(text) = fs::read_to_string(&pid_file) {
                pid = text.trim().to_string();
                if !pid.is_empty() {
                    break;
                }
            }
            sleep(Duration::from_millis(20));
        }
        assert!(
            !pid.is_empty(),
            "grandchild pidfile {} stayed empty",
            pid_file.display()
        );
        for _ in 0..100 {
            let output = Command::new("ps")
                .args(["-p", &pid, "-o", "stat=", "-o", "command="])
                .output()
                .expect("ps is required to verify process-group kill; install procps");
            let text = String::from_utf8_lossy(&output.stdout);
            let line = text.lines().next().unwrap_or("").trim();
            if line.is_empty() {
                cleanup();
                return;
            }
            let state = line.split_whitespace().next().unwrap_or("");
            if state.starts_with('Z')
                || state.starts_with('X')
                || line.contains("<defunct>")
                || !line.contains("sleep")
            {
                cleanup();
                return;
            }
            sleep(Duration::from_millis(20));
        }
        cleanup();
        panic!("grandchild sleep survived process-group kill (pid={pid})");
    }
}
