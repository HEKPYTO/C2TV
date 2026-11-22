use crate::compare::{FirstDiff, Observation};
use crate::diag::{Diagnostic, Level};
use crate::digest::Digests;
use serde::Serialize;
use std::fmt::Write as _;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ResponseReport {
    pub status: u64,
    pub ret: u64,
    pub out_len: u32,
    pub out_hex: String,
}

impl From<&Observation> for ResponseReport {
    fn from(observation: &Observation) -> Self {
        let mut out_hex = String::with_capacity(observation.out.len() * 2);
        for byte in &observation.out {
            let _ = write!(&mut out_hex, "{byte:02x}");
        }
        Self {
            status: observation.status,
            ret: observation.ret,
            out_len: observation.out.len() as u32,
            out_hex,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct LevelReport {
    pub level: &'static str,
    pub c: Option<ResponseReport>,
    pub rust: Option<ResponseReport>,
    pub verdict: &'static str,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct CaseReport {
    pub index: usize,
    pub levels: Vec<LevelReport>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct FixtureReport {
    pub id: String,
    pub kind: &'static str,
    pub expected: &'static str,
    pub observed: &'static str,
    pub classes: Vec<&'static str>,
    pub not_compared: Vec<&'static str>,
    pub digests: Digests,
    pub cases: Vec<CaseReport>,
    pub first_diff: Option<FirstDiff>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Report {
    pub schema: u32,
    pub tool: &'static str,
    pub clang_version: String,
    pub rustc_version: String,
    pub c2rust_version: Option<String>,
    pub fixtures: Vec<FixtureReport>,
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Debug)]
pub struct Pipeline {
    pub clang_version: String,
    pub rustc_version: String,
    pub c2rust_version: Option<String>,
    pub fixtures: Vec<FixtureReport>,
    pub diagnostics: Vec<Diagnostic>,
}

fn sorted(report: &Report) -> Report {
    let mut result = report.clone();
    result
        .fixtures
        .sort_by(|left, right| left.id.cmp(&right.id));
    result.diagnostics.sort();
    result
}

impl Pipeline {
    pub fn report(&self) -> Report {
        sorted(&Report {
            schema: 1,
            tool: "c2tv",
            clang_version: self.clang_version.clone(),
            rustc_version: self.rustc_version.clone(),
            c2rust_version: self.c2rust_version.clone(),
            fixtures: self.fixtures.clone(),
            diagnostics: self.diagnostics.clone(),
        })
    }
}

pub fn canonical(report: &Report) -> String {
    let mut text = serde_json::to_string(&sorted(report)).expect("report is serializable");
    text.push('\n');
    text
}

pub fn human(report: &Report) -> String {
    let report = sorted(report);
    let mut text = String::new();
    let _ = writeln!(text, "c2tv report schema={}", report.schema);
    let _ = writeln!(text, "clang: {}", report.clang_version);
    let _ = writeln!(text, "rustc: {}", report.rustc_version);
    let _ = writeln!(
        text,
        "c2rust: {}",
        report.c2rust_version.as_deref().unwrap_or("not available")
    );
    let _ = writeln!(text);
    let _ = writeln!(
        text,
        "{:<22} {:<6} {:<9} {:<9} {:<9}",
        "fixture", "kind", "expected", "observed", "o0/o2"
    );
    for fixture in &report.fixtures {
        let levels = fixture
            .cases
            .first()
            .map(|case| {
                case.levels
                    .iter()
                    .map(|level| level.verdict)
                    .collect::<Vec<_>>()
                    .join("/")
            })
            .unwrap_or_else(|| "-".to_string());
        let _ = writeln!(
            text,
            "{:<22} {:<6} {:<9} {:<9} {:<9}",
            fixture.id, fixture.kind, fixture.expected, fixture.observed, levels
        );
        if let Some(diff) = &fixture.first_diff {
            let _ = writeln!(
                text,
                "  first diff: {} byte {} c=0x{:02x} r=0x{:02x}",
                diff.field, diff.offset, diff.c, diff.r
            );
        }
        let _ = writeln!(
            text,
            "  classes: {} | not compared: {}",
            fixture.classes.join(","),
            fixture.not_compared.join(",")
        );
    }
    let _ = writeln!(text);
    let _ = writeln!(text, "diagnostics:");
    if report.diagnostics.is_empty() {
        let _ = writeln!(text, "  none");
    }
    for diagnostic in &report.diagnostics {
        let case = diagnostic
            .case
            .map(|case| case.to_string())
            .unwrap_or_else(|| "-".to_string());
        let level = diagnostic.level.map(Level::label).unwrap_or("-");
        let _ = writeln!(
            text,
            "  {} {} fixture={} case={} level={} {}",
            diagnostic.code, diagnostic.name, diagnostic.fixture, case, level, diagnostic.message
        );
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diag::{ExitClass, Severity};

    fn fixture(id: &str) -> FixtureReport {
        FixtureReport {
            id: id.to_string(),
            kind: "good",
            expected: "pass",
            observed: "pass",
            classes: vec!["ret"],
            not_compared: vec!["buf", "status"],
            digests: Digests::new(),
            cases: vec![],
            first_diff: None,
        }
    }

    fn report(ids: &[&str]) -> Report {
        Report {
            schema: 1,
            tool: "c2tv",
            clang_version: "clang 23".to_string(),
            rustc_version: "rustc 1.100".to_string(),
            c2rust_version: None,
            fixtures: ids.iter().map(|id| fixture(id)).collect(),
            diagnostics: vec![],
        }
    }

    #[test]
    fn canonical_bytes_sorted_and_stable() {
        let left = report(&["zeta", "alpha", "mid"]);
        let right = report(&["mid", "zeta", "alpha"]);
        let sorted = report(&["alpha", "mid", "zeta"]);
        let pipeline = Pipeline {
            clang_version: "clang 23".to_string(),
            rustc_version: "rustc 1.100".to_string(),
            c2rust_version: None,
            fixtures: right.fixtures.clone(),
            diagnostics: vec![],
        };
        assert_eq!(canonical(&left), canonical(&sorted));
        assert_eq!(canonical(&pipeline.report()), canonical(&sorted));
        assert!(canonical(&left).contains("\"schema\":1"));
    }

    #[test]
    fn canonical_sorts_diagnostics() {
        let mut value = report(&["a"]);
        let first = Diagnostic {
            code: "TV0009",
            name: "ub",
            severity: Severity::Warning,
            fixture: "a".to_string(),
            case: Some(2),
            level: Some(Level::O0),
            message: "z".to_string(),
            exit_class: ExitClass::Ok,
        };
        let second = Diagnostic {
            message: "a".to_string(),
            case: Some(1),
            ..first.clone()
        };
        value.diagnostics = vec![first, second];
        let pipeline = Pipeline {
            clang_version: value.clang_version.clone(),
            rustc_version: value.rustc_version.clone(),
            c2rust_version: None,
            fixtures: value.fixtures.clone(),
            diagnostics: value.diagnostics.clone(),
        };
        let text = canonical(&pipeline.report());
        assert!(text.find("\"case\":1").unwrap() < text.find("\"case\":2").unwrap());
    }

    #[test]
    fn response_report_is_lowercase_hex() {
        let observation = Observation {
            status: 0,
            ret: 0,
            out: vec![0x0a, 0xff],
        };
        assert_eq!(ResponseReport::from(&observation).out_hex, "0aff");
    }
}
