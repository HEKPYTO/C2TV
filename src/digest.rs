use crate::diag::{self, Diagnostic};
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

pub type Digests = BTreeMap<String, String>;
pub type AllDigests = BTreeMap<String, Digests>;

#[derive(Debug, Deserialize, Serialize)]
pub struct DigestFile {
    pub fixtures: AllDigests,
}

fn digest_error(fixture: &str, message: impl Into<String>) -> Diagnostic {
    Diagnostic::new(diag::DIGEST, fixture, None, None, message)
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let digest = hasher.finalize();
    let mut text = String::with_capacity(64);
    for byte in digest {
        let _ = write!(&mut text, "{byte:02x}");
    }
    text
}

fn collect(
    root: &Path,
    directory: &Path,
    files: &mut Vec<(String, PathBuf)>,
    fixture: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let entries = match fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) => {
            diagnostics.push(digest_error(
                fixture,
                format!("cannot list fixture directory: {error}"),
            ));
            return;
        }
    };
    let mut names = Vec::new();
    for entry in entries {
        match entry {
            Ok(entry) => match entry.file_type() {
                Ok(kind) => names.push((entry.file_name(), kind)),
                Err(error) => diagnostics.push(digest_error(
                    fixture,
                    format!("cannot inspect fixture entry: {error}"),
                )),
            },
            Err(error) => diagnostics.push(digest_error(
                fixture,
                format!("cannot list fixture entry: {error}"),
            )),
        }
    }
    names.sort_by(|left, right| left.0.cmp(&right.0));
    for (name, kind) in names {
        let path = directory.join(&name);
        let label = name.to_string_lossy();
        let relative = path
            .strip_prefix(root)
            .map(|value| value.to_string_lossy().replace('\\', "/"))
            .unwrap_or_else(|_| label.into_owned());
        if kind.is_symlink() {
            diagnostics.push(digest_error(
                fixture,
                format!("symlink not allowed in fixture: {relative}"),
            ));
        } else if kind.is_dir() {
            collect(root, &path, files, fixture, diagnostics);
        } else if kind.is_file() && path != root.join("digests.toml") {
            files.push((relative, path));
        } else if !kind.is_file() {
            diagnostics.push(digest_error(
                fixture,
                format!("unsupported fixture entry: {relative}"),
            ));
        }
    }
}

pub fn fixture_digests(
    directory: &Path,
    fixture: &str,
    diagnostics: &mut Vec<Diagnostic>,
) -> Digests {
    let mut files = Vec::new();
    collect(directory, directory, &mut files, fixture, diagnostics);
    files.sort_by(|left, right| left.0.cmp(&right.0));
    let mut digests = Digests::new();
    for (relative, path) in files {
        match fs::read(&path) {
            Ok(bytes) => {
                digests.insert(relative, sha256_hex(&bytes));
            }
            Err(error) => diagnostics.push(digest_error(
                fixture,
                format!("cannot read fixture input {relative}: {error}"),
            )),
        }
    }
    digests
}

pub fn read_lock(root: &Path) -> Result<Option<DigestFile>, Diagnostic> {
    let text = match fs::read_to_string(root.join("digests.toml")) {
        Ok(text) => text,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(digest_error(
                "digests.toml",
                format!("cannot read digests: {error}"),
            ));
        }
    };
    toml::from_str(&text)
        .map(Some)
        .map_err(|error| digest_error("digests.toml", format!("cannot parse digests: {error}")))
}

pub fn write_lock(root: &Path, all: &AllDigests) -> Result<(), Diagnostic> {
    let file = DigestFile {
        fixtures: all.clone(),
    };
    let text = toml::to_string_pretty(&file)
        .map_err(|error| digest_error("digests.toml", format!("cannot encode digests: {error}")))?;
    fs::write(root.join("digests.toml"), text)
        .map_err(|error| digest_error("digests.toml", format!("cannot write digests: {error}")))
}

pub fn verify(
    roots: &BTreeMap<String, PathBuf>,
    all: &AllDigests,
    lock: &DigestFile,
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    let mut ids: Vec<&String> = all.keys().chain(lock.fixtures.keys()).collect();
    ids.sort_unstable();
    ids.dedup();
    for id in ids {
        let actual = all.get(id);
        let expected = lock.fixtures.get(id);
        if let Some(expected) = expected {
            for name in expected.keys() {
                let exists = roots
                    .get(id)
                    .is_some_and(|directory| directory.join(name).is_file());
                if !exists {
                    diagnostics.push(Diagnostic::new(
                        diag::DIGEST,
                        id,
                        None,
                        None,
                        format!("locked fixture path missing: {name}"),
                    ));
                }
            }
        }
        match (actual, expected) {
            (Some(actual), Some(expected)) if actual == expected => {}
            (Some(actual), Some(expected)) => {
                let mut names: Vec<&String> = actual.keys().chain(expected.keys()).collect();
                names.sort_unstable();
                names.dedup();
                for name in names {
                    if actual.get(name) != expected.get(name) {
                        diagnostics.push(Diagnostic::new(
                            diag::DIGEST,
                            id,
                            None,
                            None,
                            format!("digest mismatch for {name}"),
                        ));
                    }
                }
            }
            (Some(_), None) => diagnostics.push(Diagnostic::new(
                diag::DIGEST,
                id,
                None,
                None,
                "discovered fixture missing from lock",
            )),
            (None, Some(_)) => diagnostics.push(Diagnostic::new(
                diag::DIGEST,
                id,
                None,
                None,
                "locked fixture id was not discovered",
            )),
            (None, None) => {}
        }
    }
    diagnostics
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_known_vector() {
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn digest_roundtrip_and_mismatch() {
        let root = std::env::temp_dir().join(format!("c2tv-verify-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("f")).unwrap();
        fs::write(root.join("f").join("port.toml"), "a").unwrap();
        let mut all = AllDigests::new();
        let mut one = Digests::new();
        one.insert("port.toml".to_string(), sha256_hex(b"a"));
        all.insert("f".to_string(), one);
        let roots: BTreeMap<String, PathBuf> =
            [("f".to_string(), root.join("f"))].into_iter().collect();
        let file = DigestFile {
            fixtures: all.clone(),
        };
        assert!(verify(&roots, &all, &file).is_empty());
        let mut other = all.clone();
        other
            .get_mut("f")
            .unwrap()
            .insert("port.toml".to_string(), "00".to_string());
        let errors = verify(&roots, &other, &file);
        assert!(!errors.is_empty());
        assert!(errors
            .iter()
            .any(|error| error.message.contains("digest mismatch")));
        let extra = DigestFile {
            fixtures: [("ghost".to_string(), all["f"].clone())]
                .into_iter()
                .collect(),
        };
        assert!(verify(&roots, &all, &extra)
            .iter()
            .any(|error| error.message.contains("locked fixture path missing")));
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn corrupt_and_unreadable_lock_are_diagnostics() {
        let root = std::env::temp_dir().join(format!("c2tv-lock-test-{}", std::process::id()));
        let _ = fs::create_dir_all(&root);
        fs::write(root.join("digests.toml"), "not toml = [").unwrap();
        let error = read_lock(&root).unwrap_err();
        assert_eq!(error.code, "TV0011");

        fs::remove_file(root.join("digests.toml")).unwrap();
        assert!(read_lock(&root).unwrap().is_none());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn digests_include_cargo_lock_and_exclude_digest_file() {
        let root = std::env::temp_dir().join(format!("c2tv-digest-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("Cargo.lock"), "lock").unwrap();
        fs::write(root.join("digests.toml"), "ignore").unwrap();
        let mut diagnostics = Vec::new();
        let digests = fixture_digests(&root, "f", &mut diagnostics);
        assert!(diagnostics.is_empty());
        assert!(digests.contains_key("Cargo.lock"));
        assert!(!digests.contains_key("digests.toml"));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn unreadable_inputs_and_directories_are_diagnostics() {
        let mut diagnostics = Vec::new();
        fixture_digests(
            Path::new("/nonexistent/c2tv-fixture"),
            "f",
            &mut diagnostics,
        );
        assert_eq!(diagnostics[0].code, "TV0011");

        let root = std::env::temp_dir().join(format!("c2tv-unreadable-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let file = root.join("blocked");
        fs::write(&file, "x").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&file, fs::Permissions::from_mode(0o000)).unwrap();
        }
        let mut diagnostics = Vec::new();
        fixture_digests(&root, "f", &mut diagnostics);
        assert!(diagnostics.iter().any(|item| item.code == "TV0011"));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&file, fs::Permissions::from_mode(0o600)).unwrap();
        }
        let _ = fs::remove_dir_all(&root);
    }
    #[cfg(unix)]
    #[test]
    fn symlink_loop_and_symlinked_file_are_rejected() {
        use std::os::unix::fs::symlink;
        let root = std::env::temp_dir().join(format!("c2tv-digest-symlink-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("sub")).unwrap();
        fs::write(root.join("sub").join("ok.txt"), "ok").unwrap();
        symlink("..", root.join("sub").join("loop")).unwrap();
        symlink(root.join("sub").join("ok.txt"), root.join("link.txt")).unwrap();
        let mut diagnostics = Vec::new();
        let digests = fixture_digests(&root, "f", &mut diagnostics);
        assert!(digests.contains_key("sub/ok.txt"));
        assert!(!digests.contains_key("sub/loop"));
        assert!(!digests.contains_key("link.txt"));
        assert_eq!(
            diagnostics
                .iter()
                .filter(|item| item.message.contains("symlink not allowed"))
                .count(),
            2
        );
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn nested_digest_lock_is_pinned() {
        let root = std::env::temp_dir().join(format!("c2tv-digest-nested-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("sub")).unwrap();
        fs::write(root.join("digests.toml"), "root lock").unwrap();
        fs::write(root.join("sub").join("digests.toml"), "nested lock").unwrap();
        let mut diagnostics = Vec::new();
        let digests = fixture_digests(&root, "f", &mut diagnostics);
        assert!(diagnostics.is_empty());
        assert!(!digests.contains_key("digests.toml"));
        assert_eq!(digests["sub/digests.toml"], sha256_hex(b"nested lock"));
        fs::remove_dir_all(&root).unwrap();
    }
}
