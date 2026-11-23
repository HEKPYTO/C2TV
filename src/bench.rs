use crate::diag::ExitClass;
use crate::report::FixtureReport;
use serde::Serialize;
use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::time::Duration;

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct Counts {
    pub total: usize,
    pub good: usize,
    pub bad: usize,
    pub real: usize,
    pub true_positive: usize,
    pub false_negative: usize,
    pub true_negative: usize,
    pub false_positive: usize,
    pub tool_errors: usize,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Rate {
    pub numerator: usize,
    pub denominator: usize,
    pub rate: f64,
    pub denominator_basis: &'static str,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Rates {
    pub false_positive: Rate,
    pub false_negative: Rate,
}

#[derive(Debug, Serialize)]
pub struct Report {
    pub schema: u32,
    pub clang_version: String,
    pub rustc_version: String,
    pub c2rust_version: Option<String>,
    pub counts: Counts,
    pub rates: Rates,
}

pub fn rate(numerator: usize, denominator: usize, denominator_basis: &'static str) -> Rate {
    let value = if denominator == 0 {
        0.0
    } else {
        numerator as f64 / denominator as f64
    };
    Rate {
        numerator,
        denominator,
        rate: value,
        denominator_basis,
    }
}

pub fn rates(counts: &Counts) -> Rates {
    Rates {
        false_positive: rate(
            counts.false_positive,
            counts.true_negative + counts.false_positive,
            "true_negative + false_positive",
        ),
        false_negative: rate(
            counts.false_negative,
            counts.true_positive + counts.false_negative,
            "true_positive + false_negative",
        ),
    }
}

pub fn measure(fixtures: &[FixtureReport], diagnostics: &[crate::diag::Diagnostic]) -> Counts {
    let errored: BTreeSet<&str> = diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.exit_class == ExitClass::ToolError)
        .map(|diagnostic| diagnostic.fixture.as_str())
        .collect();
    let mut counts = Counts::default();
    for fixture in fixtures {
        counts.total += 1;
        match fixture.kind {
            "good" => counts.good += 1,
            "bad" => counts.bad += 1,
            "real" => counts.real += 1,
            _ => {}
        }
        if errored.contains("c2tv") || errored.contains(fixture.id.as_str()) {
            continue;
        }
        let matched = crate::expectation_diagnostics(std::slice::from_ref(fixture)).is_empty();
        match fixture.kind {
            "bad" if matched => counts.true_positive += 1,
            "bad" => counts.false_negative += 1,
            "good" | "real" if matched => counts.true_negative += 1,
            "good" | "real" => counts.false_positive += 1,
            _ => {}
        }
    }
    counts.tool_errors = errored.len();
    counts
}

pub fn canonical(report: &Report) -> String {
    let mut text = serde_json::to_string(report).expect("bench report is serializable");
    text.push('\n');
    text
}

pub fn human(report: &Report, elapsed: Duration) -> String {
    let mut text = String::new();
    let _ = writeln!(text, "c2tv bench schema={}", report.schema);
    let _ = writeln!(text, "clang: {}", report.clang_version);
    let _ = writeln!(text, "rustc: {}", report.rustc_version);
    let _ = writeln!(
        text,
        "c2rust: {}",
        report.c2rust_version.as_deref().unwrap_or("not available")
    );
    let _ = writeln!(text, "elapsed: {}ms", elapsed.as_millis());
    let _ = writeln!(text);
    let _ = writeln!(text, "{:<24} {:>10}", "metric", "count");
    let counts = &report.counts;
    for (name, value) in [
        ("total", counts.total),
        ("good", counts.good),
        ("bad", counts.bad),
        ("real", counts.real),
        ("true positives", counts.true_positive),
        ("false negatives", counts.false_negative),
        ("true negatives", counts.true_negative),
        ("false positives", counts.false_positive),
        ("tool errors", counts.tool_errors),
    ] {
        let _ = writeln!(text, "{name:<24} {value:>10}");
    }
    let _ = writeln!(text);
    let _ = writeln!(text, "{:<24} {:>10}", "rate", "value");
    for (name, value) in [
        ("false positive", &report.rates.false_positive),
        ("false negative", &report.rates.false_negative),
    ] {
        let _ = writeln!(
            text,
            "{name:<24} {:>10}",
            format!(
                "{}/{} ({:.4})",
                value.numerator, value.denominator, value.rate
            )
        );
        let _ = writeln!(text, "  denominator: {}", value.denominator_basis);
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diag::Diagnostic;
    use crate::diag::SPAWN;
    use crate::digest::Digests;

    fn fixture(id: &str, kind: &'static str, observed: &'static str) -> FixtureReport {
        let expected = if kind == "bad" { "diverge" } else { "pass" };
        FixtureReport {
            id: id.to_string(),
            kind,
            expected,
            observed,
            classes: vec!["ret"],
            not_compared: vec![],
            digests: Digests::new(),
            cases: vec![],
            first_diff: None,
            first_diff_case: None,
            expect_case: None,
            expect_class: None,
        }
    }

    #[test]
    fn rate_math_handles_zero_denominator() {
        let empty = rate(0, 0, "none");
        assert_eq!(empty.numerator, 0);
        assert_eq!(empty.denominator, 0);
        assert_eq!(empty.rate, 0.0);
        let quarter = rate(1, 4, "all");
        assert_eq!(quarter.rate, 0.25);
    }

    #[test]
    fn rates_use_declared_denominators() {
        let counts = Counts {
            true_positive: 3,
            false_negative: 1,
            true_negative: 21,
            false_positive: 2,
            ..Counts::default()
        };
        let values = rates(&counts);
        assert_eq!(values.false_positive.numerator, 2);
        assert_eq!(values.false_positive.denominator, 23);
        assert_eq!(values.false_negative.denominator, 4);
        assert_eq!(values.false_negative.rate, 0.25);
    }

    #[test]
    fn tool_errors_are_excluded_from_rates() {
        let fixtures = vec![
            fixture("good", "good", "pass"),
            fixture("bad", "bad", "diverge"),
            fixture("errored", "good", "diverge"),
        ];
        let diagnostics = vec![Diagnostic::new(
            SPAWN,
            "errored",
            None,
            None,
            "cannot start harness",
        )];
        let counts = measure(&fixtures, &diagnostics);
        assert_eq!(counts.total, 3);
        assert_eq!(counts.true_negative, 1);
        assert_eq!(counts.true_positive, 1);
        assert_eq!(counts.false_positive, 0);
        assert_eq!(counts.tool_errors, 1);
        assert_eq!(rates(&counts).false_positive.denominator, 1);
    }

    #[test]
    fn global_tool_error_excludes_every_fixture() {
        let fixtures = vec![
            fixture("good", "good", "pass"),
            fixture("bad", "bad", "diverge"),
        ];
        let diagnostics = vec![Diagnostic::new(SPAWN, "c2tv", None, None, "no work root")];
        let counts = measure(&fixtures, &diagnostics);
        assert_eq!(counts.total, 2);
        assert_eq!(counts.true_positive, 0);
        assert_eq!(counts.true_negative, 0);
        assert_eq!(counts.tool_errors, 1);
        assert_eq!(rates(&counts).false_negative.denominator, 0);
    }

    #[test]
    fn wrong_divergence_reason_is_a_false_negative() {
        let mut bad = fixture("bad", "bad", "diverge");
        bad.expect_case = Some(0);
        bad.expect_class = Some("ret");
        bad.first_diff_case = Some(1);
        let counts = measure(std::slice::from_ref(&bad), &[]);
        assert_eq!(counts.false_negative, 1);
        assert_eq!(counts.true_positive, 0);
    }
}
