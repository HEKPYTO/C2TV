use crate::diag::{self, Diagnostic, ExitClass, Severity};
use crate::process;
use crate::transpile;
use serde::Serialize;
use std::fmt::Write as _;
use std::fs;
use std::io::ErrorKind;
use std::path::Path;

const C2TV_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ToolStatus {
    pub present: bool,
    pub version: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct DoctorReport {
    pub schema: u32,
    pub tool: &'static str,
    pub c2tv_version: &'static str,
    pub rustc: ToolStatus,
    pub cargo: ToolStatus,
    pub clang: ToolStatus,
    pub c2rust: ToolStatus,
    pub toolchain_channel: Option<String>,
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProbeOutcome {
    Version(String),
    Missing,
    Spawn,
    Timeout,
    Nonzero,
}

pub trait Prober {
    fn probe(&self, program: &str, args: &[&str]) -> ProbeOutcome;
}

pub struct SystemProber;

impl Prober for SystemProber {
    fn probe(&self, program: &str, args: &[&str]) -> ProbeOutcome {
        let mut command = process::sanitized(program);
        command.args(args);
        match process::run(&mut command, None, process::PROBE_TIMEOUT) {
            Err(error) if error.kind() == ErrorKind::NotFound => ProbeOutcome::Missing,
            Err(_) => ProbeOutcome::Spawn,
            Ok(captured) if captured.timed_out => ProbeOutcome::Timeout,
            Ok(captured) if !captured.success => ProbeOutcome::Nonzero,
            Ok(captured) => {
                let version = String::from_utf8_lossy(&captured.stdout)
                    .lines()
                    .next()
                    .unwrap_or("")
                    .trim()
                    .to_string();
                if version.is_empty() {
                    ProbeOutcome::Nonzero
                } else {
                    ProbeOutcome::Version(version)
                }
            }
        }
    }
}

fn status(outcome: &ProbeOutcome) -> ToolStatus {
    match outcome {
        ProbeOutcome::Version(version) => ToolStatus {
            present: true,
            version: Some(version.clone()),
        },
        _ => ToolStatus {
            present: false,
            version: None,
        },
    }
}

fn failure(outcome: &ProbeOutcome, label: &str, diagnostics: &mut Vec<Diagnostic>) {
    let (info, message) = match outcome {
        ProbeOutcome::Missing => (diag::DOCTOR_MISSING_TOOL, format!("{label} not found")),
        ProbeOutcome::Spawn => (diag::SPAWN, format!("cannot probe {label}")),
        ProbeOutcome::Timeout => (diag::TIMEOUT, format!("probing {label} timed out")),
        ProbeOutcome::Nonzero => (diag::NONZERO_EXIT, format!("{label} version probe failed")),
        ProbeOutcome::Version(_) => return,
    };
    diagnostics.push(Diagnostic::new(info, "c2tv", None, None, message));
}

fn required(prober: &dyn Prober, program: &str, diagnostics: &mut Vec<Diagnostic>) -> ToolStatus {
    let outcome = prober.probe(program, &["--version"]);
    let result = status(&outcome);
    failure(&outcome, program, diagnostics);
    result
}

fn toolchain_file_error(diagnostics: &mut Vec<Diagnostic>, message: &str) {
    diagnostics.push(Diagnostic::new(
        diag::TOOLCHAIN_FILE,
        "c2tv",
        None,
        None,
        message,
    ));
}

fn channel(root: &Path, diagnostics: &mut Vec<Diagnostic>) -> Option<String> {
    let text = match fs::read_to_string(root.join("rust-toolchain.toml")) {
        Ok(text) => text,
        Err(error) if error.kind() == ErrorKind::NotFound => return None,
        Err(_) => {
            toolchain_file_error(diagnostics, "cannot read rust-toolchain.toml");
            return None;
        }
    };
    let value: toml::Value = match toml::from_str(&text) {
        Ok(value) => value,
        Err(_) => {
            toolchain_file_error(diagnostics, "rust-toolchain.toml is not valid TOML");
            return None;
        }
    };
    let Some(toolchain) = value.get("toolchain") else {
        toolchain_file_error(diagnostics, "rust-toolchain.toml has no toolchain table");
        return None;
    };
    let Some(channel) = toolchain.get("channel") else {
        toolchain_file_error(diagnostics, "rust-toolchain.toml has no toolchain.channel");
        return None;
    };
    let Some(channel) = channel.as_str() else {
        toolchain_file_error(diagnostics, "rust-toolchain.toml channel is not a string");
        return None;
    };
    let channel = channel.trim();
    if channel.is_empty() {
        toolchain_file_error(diagnostics, "rust-toolchain.toml channel is empty");
        return None;
    }
    Some(channel.to_string())
}

fn drift(
    channel: Option<&str>,
    rustc: Option<&str>,
    strict: bool,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let (Some(channel), Some(rustc)) = (channel, rustc) else {
        return;
    };
    let Some(actual) = rustc
        .strip_prefix("rustc ")
        .and_then(|text| text.split_whitespace().next())
    else {
        return;
    };
    if !channel.starts_with(|character: char| character.is_ascii_digit()) || channel == actual {
        return;
    }
    let mut diagnostic = Diagnostic::new(
        diag::TOOLCHAIN_DRIFT,
        "c2tv",
        None,
        None,
        format!("rust-toolchain.toml channel {channel} differs from rustc {actual}"),
    );
    if strict {
        diagnostic.severity = Severity::Error;
        diagnostic.exit_class = ExitClass::ToolError;
    }
    diagnostics.push(diagnostic);
}

pub fn collect(prober: &dyn Prober, root: &Path, strict: bool, transpile: bool) -> DoctorReport {
    let mut diagnostics = Vec::new();
    let rustc = required(prober, "rustc", &mut diagnostics);
    let cargo = required(prober, "cargo", &mut diagnostics);
    let clang = required(prober, "clang", &mut diagnostics);

    let c2rust_probe = prober.probe("c2rust", &["--version"]);
    let c2rust = status(&c2rust_probe);
    match &c2rust_probe {
        ProbeOutcome::Version(_) => {}
        ProbeOutcome::Missing if transpile => {}
        ProbeOutcome::Missing => diagnostics.push(Diagnostic::new(
            diag::CAPABILITY,
            "c2tv",
            None,
            None,
            "c2rust not available on PATH",
        )),
        other => failure(other, "c2rust", &mut diagnostics),
    }
    if transpile {
        let pin = prober.probe("c2rust", &["transpile", "--version"]);
        match &pin {
            ProbeOutcome::Version(_) | ProbeOutcome::Missing => {}
            other => failure(other, "c2rust transpile", &mut diagnostics),
        }
        let version = match pin {
            ProbeOutcome::Version(version) => Some(version),
            _ => None,
        };
        if let Err(error) = transpile::check_version(version) {
            diagnostics.push(error);
        }
    }

    let channel = channel(root, &mut diagnostics);
    drift(
        channel.as_deref(),
        rustc.version.as_deref(),
        strict,
        &mut diagnostics,
    );
    diagnostics.sort();
    DoctorReport {
        schema: 1,
        tool: "c2tv",
        c2tv_version: C2TV_VERSION,
        rustc,
        cargo,
        clang,
        c2rust,
        toolchain_channel: channel,
        diagnostics,
    }
}

pub fn canonical(report: &DoctorReport) -> String {
    let mut text = serde_json::to_string(report).expect("doctor report is serializable");
    text.push('\n');
    text
}

fn severity_label(severity: Severity) -> &'static str {
    match severity {
        Severity::Error => "error",
        Severity::Info => "info",
        Severity::Warning => "warning",
    }
}

pub fn human(report: &DoctorReport) -> String {
    let mut text = String::new();
    let _ = writeln!(text, "c2tv doctor schema={}", report.schema);
    let _ = writeln!(text, "{:<10} {:<9} version", "tool", "status");
    let rows = [
        ("c2tv", Some(report.c2tv_version)),
        ("rustc", report.rustc.version.as_deref()),
        ("cargo", report.cargo.version.as_deref()),
        ("clang", report.clang.version.as_deref()),
        ("c2rust", report.c2rust.version.as_deref()),
    ];
    for (name, version) in rows {
        let status = if version.is_some() { "ok" } else { "missing" };
        let _ = writeln!(text, "{name:<10} {status:<9} {}", version.unwrap_or("-"));
    }
    let _ = writeln!(
        text,
        "toolchain channel: {}",
        report.toolchain_channel.as_deref().unwrap_or("-")
    );
    if !report.diagnostics.is_empty() {
        let _ = writeln!(text);
        let _ = writeln!(text, "diagnostics:");
        for diagnostic in &report.diagnostics {
            let _ = writeln!(
                text,
                "  {} {} {}: {}",
                diagnostic.code,
                severity_label(diagnostic.severity),
                diagnostic.name,
                diagnostic.message
            );
        }
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    #[derive(Default)]
    struct FakeProber {
        outcomes: BTreeMap<(String, String), ProbeOutcome>,
    }

    impl FakeProber {
        fn set(&mut self, program: &str, args: &[&str], outcome: ProbeOutcome) {
            self.outcomes
                .insert((program.to_string(), args.join(" ")), outcome);
        }
    }

    impl Prober for FakeProber {
        fn probe(&self, program: &str, args: &[&str]) -> ProbeOutcome {
            self.outcomes
                .get(&(program.to_string(), args.join(" ")))
                .cloned()
                .unwrap_or(ProbeOutcome::Missing)
        }
    }

    fn healthy() -> FakeProber {
        let mut prober = FakeProber::default();
        prober.set(
            "rustc",
            &["--version"],
            ProbeOutcome::Version("rustc 1.100.0 (0ef1bc386 2026-11-09)".into()),
        );
        prober.set(
            "cargo",
            &["--version"],
            ProbeOutcome::Version("cargo 1.100.0 (3acc79c0c 2026-11-05)".into()),
        );
        prober.set(
            "clang",
            &["--version"],
            ProbeOutcome::Version("clang version 18.1.3".into()),
        );
        prober.set(
            "c2rust",
            &["--version"],
            ProbeOutcome::Version("C2Rust 0.22.1".into()),
        );
        prober.set(
            "c2rust",
            &["transpile", "--version"],
            ProbeOutcome::Version("transpile 0.22.1".into()),
        );
        prober
    }

    fn root(label: &str, channel: Option<&str>) -> PathBuf {
        let root = std::env::temp_dir().join(format!("c2tv-doctor-{label}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        if let Some(channel) = channel {
            fs::write(
                root.join("rust-toolchain.toml"),
                format!("[toolchain]\nchannel = \"{channel}\"\ncomponents = [\"rustfmt\"]\n"),
            )
            .unwrap();
        }
        root
    }

    fn codes(report: &DoctorReport) -> Vec<&str> {
        report.diagnostics.iter().map(|item| item.code).collect()
    }

    #[test]
    fn missing_required_tool_is_typed_tool_error() {
        let root = root("missing", None);
        let mut prober = healthy();
        prober.set("rustc", &["--version"], ProbeOutcome::Missing);
        let report = collect(&prober, &root, false, false);
        assert_eq!(codes(&report), vec!["TV0023"]);
        assert_eq!(diag::exit_code(&report.diagnostics), 2);
        assert!(!report.rustc.present);
        assert!(report.diagnostics[0].message.contains("rustc"));

        let mut prober = healthy();
        prober.set("clang", &["--version"], ProbeOutcome::Missing);
        let report = collect(&prober, &root, false, false);
        assert_eq!(codes(&report), vec!["TV0023"]);
        assert_eq!(diag::exit_code(&report.diagnostics), 2);
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn probe_failures_are_typed() {
        let root = root("failures", None);
        for (outcome, code) in [
            (ProbeOutcome::Spawn, "TV0005"),
            (ProbeOutcome::Timeout, "TV0006"),
            (ProbeOutcome::Nonzero, "TV0007"),
        ] {
            let mut prober = healthy();
            prober.set("clang", &["--version"], outcome);
            let report = collect(&prober, &root, false, false);
            assert_eq!(codes(&report), vec![code]);
            assert_eq!(diag::exit_code(&report.diagnostics), 2);
        }
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn c2rust_absence_is_info_unless_transpile() {
        let root = root("c2rust", None);
        let mut prober = healthy();
        prober.set("c2rust", &["--version"], ProbeOutcome::Missing);
        prober.set("c2rust", &["transpile", "--version"], ProbeOutcome::Missing);
        let report = collect(&prober, &root, false, false);
        assert_eq!(codes(&report), vec!["TV0014"]);
        assert_eq!(report.diagnostics[0].severity, Severity::Info);
        assert_eq!(diag::exit_code(&report.diagnostics), 0);

        let report = collect(&prober, &root, false, true);
        assert_eq!(codes(&report), vec!["TV0016"]);
        assert_eq!(diag::exit_code(&report.diagnostics), 2);
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn strict_changes_drift_exit_class() {
        let drift_root = root("drift", Some("9.9.9"));
        let prober = healthy();
        let report = collect(&prober, &drift_root, false, false);
        assert_eq!(codes(&report), vec!["TV0024"]);
        assert_eq!(report.diagnostics[0].severity, Severity::Warning);
        assert_eq!(diag::exit_code(&report.diagnostics), 0);

        let report = collect(&prober, &drift_root, true, false);
        assert_eq!(codes(&report), vec!["TV0024"]);
        assert_eq!(report.diagnostics[0].severity, Severity::Error);
        assert_eq!(diag::exit_code(&report.diagnostics), 2);
        fs::remove_dir_all(&drift_root).unwrap();

        let matching = root("match", Some("1.100.0"));
        let report = collect(&prober, &matching, true, true);
        assert!(report.diagnostics.is_empty());
        assert_eq!(diag::exit_code(&report.diagnostics), 0);
        fs::remove_dir_all(&matching).unwrap();

        let named = root("named", Some("stable"));
        let report = collect(&prober, &named, true, true);
        assert!(report.diagnostics.is_empty());
        fs::remove_dir_all(&named).unwrap();
    }

    #[test]
    fn invalid_toolchain_file_fails_closed() {
        let root = root("invalid", None);
        fs::write(root.join("rust-toolchain.toml"), "[toolchain\n").unwrap();
        let report = collect(&healthy(), &root, false, false);
        assert_eq!(codes(&report), vec!["TV0025"]);
        assert_eq!(diag::exit_code(&report.diagnostics), 2);
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn present_toolchain_file_requires_valid_channel() {
        for (label, text) in [
            ("no_table", "channel = \"1.100.0\"\n"),
            ("missing_key", "[toolchain]\ncomponents = [\"rustfmt\"]\n"),
            ("wrong_type", "[toolchain]\nchannel = 100\n"),
            ("empty", "[toolchain]\nchannel = \"\"\n"),
            ("whitespace", "[toolchain]\nchannel = \"   \"\n"),
        ] {
            let root = root(label, None);
            fs::write(root.join("rust-toolchain.toml"), text).unwrap();
            for (strict, transpile) in [(false, false), (true, false), (true, true)] {
                let report = collect(&healthy(), &root, strict, transpile);
                assert_eq!(codes(&report), vec!["TV0025"], "label={label}");
                assert_eq!(diag::exit_code(&report.diagnostics), 2, "label={label}");
                assert!(report.toolchain_channel.is_none(), "label={label}");
            }
            fs::remove_dir_all(&root).unwrap();
        }
    }

    #[test]
    fn trimmed_channel_is_reported_without_drift() {
        let root = root("trim", None);
        fs::write(
            root.join("rust-toolchain.toml"),
            "[toolchain]\nchannel = \"  1.100.0  \"\n",
        )
        .unwrap();
        let report = collect(&healthy(), &root, true, true);
        assert_eq!(report.toolchain_channel.as_deref(), Some("1.100.0"));
        assert!(report.diagnostics.is_empty());
        assert_eq!(diag::exit_code(&report.diagnostics), 0);
        fs::remove_dir_all(&root).unwrap();
    }
    #[test]
    fn transpile_pin_requires_exact_version() {
        let root = root("pin", None);
        for version in ["transpile 0.22.2", "c2rust 0.22.1", "transpile 0.22.1 "] {
            let mut prober = healthy();
            prober.set(
                "c2rust",
                &["transpile", "--version"],
                ProbeOutcome::Version(version.into()),
            );
            let report = collect(&prober, &root, false, true);
            assert_eq!(codes(&report), vec!["TV0016"], "version={version}");
            assert_eq!(diag::exit_code(&report.diagnostics), 2);
        }
        let mut prober = healthy();
        prober.set("c2rust", &["transpile", "--version"], ProbeOutcome::Timeout);
        let report = collect(&prober, &root, false, true);
        assert_eq!(codes(&report), vec!["TV0006", "TV0016"]);
        assert_eq!(diag::exit_code(&report.diagnostics), 2);
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn json_is_deterministic_and_path_free() {
        let root = root("json", Some("1.100.0"));
        let prober = healthy();
        let first = canonical(&collect(&prober, &root, true, true));
        let second = canonical(&collect(&prober, &root, true, true));
        assert_eq!(first, second);
        assert!(!first.contains(&root.display().to_string()));
        assert!(!first.contains(std::env::temp_dir().to_str().unwrap()));
        let value: serde_json::Value = serde_json::from_str(&first).unwrap();
        assert_eq!(value["schema"], 1);
        assert_eq!(value["c2tv_version"], env!("CARGO_PKG_VERSION"));
        assert!(value["rustc"]["present"].as_bool().unwrap());
        assert!(value["diagnostics"].as_array().unwrap().is_empty());
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn human_output_has_tool_table() {
        let root = root("human", None);
        let text = human(&collect(&healthy(), &root, false, false));
        let header: Vec<&str> = text.lines().nth(1).unwrap().split_whitespace().collect();
        assert_eq!(header, vec!["tool", "status", "version"]);
        for name in ["rustc", "cargo", "clang", "c2rust"] {
            let row = text.lines().find(|line| line.starts_with(name)).unwrap();
            let cells: Vec<&str> = row.split_whitespace().collect();
            assert_eq!(cells[0], name);
            assert_eq!(cells[1], "ok");
        }
        fs::remove_dir_all(&root).unwrap();
    }
}
