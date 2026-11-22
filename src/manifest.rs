use crate::abi;

use crate::diag::{self, Diagnostic};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Kind {
    Good,
    Bad,
    Real,
}

impl Kind {
    pub fn label(self) -> &'static str {
        match self {
            Kind::Good => "good",
            Kind::Bad => "bad",
            Kind::Real => "real",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Verdict {
    Pass,
    Diverge,
}

impl Verdict {
    pub fn label(self) -> &'static str {
        match self {
            Verdict::Pass => "pass",
            Verdict::Diverge => "diverge",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Class {
    Buf,
    Err,
    Ret,
}

impl Class {
    fn parse(text: &str) -> Option<Self> {
        match text {
            "buf" => Some(Class::Buf),
            "err" => Some(Class::Err),
            "ret" => Some(Class::Ret),
            _ => None,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Class::Buf => "buf",
            Class::Err => "err",
            Class::Ret => "ret",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Manifest {
    pub kind: Kind,
    pub classes: Vec<Class>,
    pub expected: Verdict,
    pub transpile: bool,
    pub expect_case: Option<usize>,
    pub expect_class: Option<Class>,
}
const KEYS: [&str; 10] = [
    "schema",
    "id",
    "kind",
    "classes",
    "expected",
    "bug",
    "note",
    "transpile",
    "expect_case",
    "expect_class",
];
const BUGS: [&str; 3] = ["overflow", "aliasing", "off-by-one"];

fn bad(fixture: &str, message: impl Into<String>) -> Diagnostic {
    Diagnostic::new(diag::MANIFEST_VALUE, fixture, None, None, message)
}

pub fn parse_manifest(
    text: &str,
    dir_name: &str,
    fixture: &str,
) -> Result<Manifest, Vec<Diagnostic>> {
    let mut errors = Vec::new();
    let value: toml::Value = match toml::from_str(text) {
        Ok(value) => value,
        Err(error) => {
            errors.push(Diagnostic::new(
                diag::MANIFEST_PARSE,
                fixture,
                None,
                None,
                format!("invalid TOML: {error}"),
            ));
            return Err(errors);
        }
    };
    let table = match value.as_table() {
        Some(table) => table,
        None => {
            errors.push(Diagnostic::new(
                diag::MANIFEST_PARSE,
                fixture,
                None,
                None,
                "manifest root must be a table",
            ));
            return Err(errors);
        }
    };

    let mut unknown: Vec<&str> = table
        .keys()
        .map(String::as_str)
        .filter(|key| !KEYS.contains(key))
        .collect();
    unknown.sort_unstable();
    for key in unknown {
        errors.push(Diagnostic::new(
            diag::UNKNOWN_KEY,
            fixture,
            None,
            None,
            format!("unknown manifest key: {key}"),
        ));
    }

    match table.get("schema").and_then(toml::Value::as_integer) {
        Some(1) => {}
        Some(value) => errors.push(bad(fixture, format!("schema must be 1, got {value}"))),
        None => errors.push(bad(fixture, "schema must be the integer 1")),
    }

    let _id = match table.get("id").and_then(toml::Value::as_str) {
        Some(id) => {
            if id != dir_name {
                errors.push(bad(
                    fixture,
                    format!("id {id} does not match directory name {dir_name}"),
                ));
            }
            id.to_string()
        }
        None => {
            errors.push(bad(fixture, "id must be a string"));
            String::new()
        }
    };

    let kind = match table.get("kind").and_then(toml::Value::as_str) {
        Some("good") => Kind::Good,
        Some("bad") => Kind::Bad,
        Some("real") => Kind::Real,
        Some(value) => {
            errors.push(bad(fixture, format!("invalid kind: {value}")));
            Kind::Good
        }
        None => {
            errors.push(bad(fixture, "kind must be good, bad, or real"));
            Kind::Good
        }
    };

    let mut classes = Vec::new();
    match table.get("classes").and_then(toml::Value::as_array) {
        Some(array) => {
            for (index, item) in array.iter().enumerate() {
                match item.as_str().and_then(Class::parse) {
                    Some(class) if !classes.contains(&class) => classes.push(class),
                    Some(_) => {}
                    None => errors.push(bad(
                        fixture,
                        format!("classes[{index}] must be buf, err, or ret"),
                    )),
                }
            }
        }
        None => errors.push(bad(fixture, "classes must be an array of strings")),
    }
    classes.sort_unstable();
    if classes.is_empty() {
        errors.push(bad(fixture, "classes must not be empty"));
    }

    let expected = match table.get("expected").and_then(toml::Value::as_str) {
        Some("pass") => Verdict::Pass,
        Some("diverge") => Verdict::Diverge,
        Some(value) => {
            errors.push(bad(fixture, format!("invalid expected: {value}")));
            Verdict::Pass
        }
        None => {
            errors.push(bad(fixture, "expected must be pass or diverge"));
            Verdict::Pass
        }
    };

    if kind == Kind::Real && expected != Verdict::Pass {
        errors.push(bad(fixture, "real fixtures must expect pass"));
    }

    let _bug = match table.get("bug") {
        Some(_) if kind != Kind::Bad => {
            errors.push(bad(fixture, "bug is only valid for bad fixtures"));
            None
        }
        Some(value) => match value.as_str() {
            Some(name) if BUGS.contains(&name) => Some(name.to_string()),
            Some(name) => {
                errors.push(bad(fixture, format!("invalid bug: {name}")));
                None
            }
            None => {
                errors.push(bad(fixture, "bug must be a string"));
                None
            }
        },
        None if kind == Kind::Bad => {
            errors.push(bad(fixture, "bug is required for bad fixtures"));
            None
        }
        None => None,
    };

    let expect_case = match table.get("expect_case") {
        Some(_) if kind != Kind::Bad => {
            errors.push(bad(fixture, "expect_case is only valid for bad fixtures"));
            None
        }
        Some(value) => match value.as_integer() {
            Some(number) if number >= 0 => Some(number as usize),
            _ => {
                errors.push(bad(fixture, "expect_case must be a non-negative integer"));
                None
            }
        },
        None if kind == Kind::Bad => {
            errors.push(bad(fixture, "expect_case is required for bad fixtures"));
            None
        }
        None => None,
    };
    let expect_class = match table.get("expect_class") {
        Some(_) if kind != Kind::Bad => {
            errors.push(bad(fixture, "expect_class is only valid for bad fixtures"));
            None
        }
        Some(value) => match value.as_str().and_then(Class::parse) {
            Some(class) => Some(class),
            None => {
                errors.push(bad(fixture, "expect_class must be ret, buf, or err"));
                None
            }
        },
        None if kind == Kind::Bad => {
            errors.push(bad(fixture, "expect_class is required for bad fixtures"));
            None
        }
        None => None,
    };

    let _note = match table.get("note").and_then(toml::Value::as_str) {
        Some(note) if !note.contains('\n') && !note.contains('\r') => note.to_string(),
        Some(_) => {
            errors.push(bad(fixture, "note must be one line"));
            String::new()
        }
        None => {
            errors.push(bad(fixture, "note must be a string"));
            String::new()
        }
    };

    let transpile = match table.get("transpile") {
        Some(value) => match value.as_bool() {
            Some(true) if kind == Kind::Bad => {
                errors.push(bad(
                    fixture,
                    "transpile is only valid for good and real fixtures",
                ));
                false
            }
            Some(flag) => flag,
            None => {
                errors.push(bad(fixture, "transpile must be a boolean"));
                false
            }
        },
        None => false,
    };

    if errors.is_empty() {
        Ok(Manifest {
            kind,
            classes,
            expected,
            transpile,
            expect_case,
            expect_class,
        })
    } else {
        Err(errors)
    }
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

pub fn parse_corpus(text: &str, fixture: &str) -> Result<Vec<Vec<u8>>, Vec<Diagnostic>> {
    let mut cases = Vec::new();
    let mut errors = Vec::new();
    for (index, raw) in text.lines().enumerate() {
        let line = raw.trim_end_matches('\r');
        if line == "-" {
            cases.push(Vec::new());
            continue;
        }
        if line.is_empty() {
            errors.push(Diagnostic::new(
                diag::CORPUS_HEX,
                fixture,
                Some(index),
                None,
                "empty corpus line; use - for the empty input",
            ));
            continue;
        }
        if line.len() % 2 != 0 {
            errors.push(Diagnostic::new(
                diag::CORPUS_HEX,
                fixture,
                Some(index),
                None,
                "corpus line has an odd number of hex digits",
            ));
            continue;
        }
        if line.len() / 2 > abi::REQUEST_CAP {
            errors.push(Diagnostic::new(
                diag::CORPUS_HEX,
                fixture,
                Some(index),
                None,
                format!("corpus line exceeds {}-byte request cap", abi::REQUEST_CAP),
            ));
            continue;
        }
        let mut case = Vec::with_capacity(line.len() / 2);
        let mut valid = true;
        for pair in line.as_bytes().chunks(2) {
            match (hex_value(pair[0]), hex_value(pair[1])) {
                (Some(high), Some(low)) => case.push(high * 16 + low),
                _ => {
                    valid = false;
                    break;
                }
            }
        }
        if valid {
            cases.push(case);
        } else {
            errors.push(Diagnostic::new(
                diag::CORPUS_HEX,
                fixture,
                Some(index),
                None,
                "corpus line contains a non-hex digit",
            ));
        }
    }
    if cases.is_empty() {
        errors.push(Diagnostic::new(
            diag::CORPUS_HEX,
            fixture,
            None,
            None,
            "corpus must contain at least one case",
        ));
    }
    if errors.is_empty() {
        Ok(cases)
    } else {
        Err(errors)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEXT: &str = "schema = 1\nid = \"varint\"\nkind = \"good\"\nclasses = [\"ret\", \"buf\"]\nexpected = \"pass\"\nnote = \"loop edges\"\n";

    #[test]
    fn parses_valid_manifest() {
        let manifest = parse_manifest(TEXT, "varint", "varint").unwrap();
        assert_eq!(manifest.kind, Kind::Good);
        assert_eq!(manifest.classes, vec![Class::Buf, Class::Ret]);
        assert_eq!(manifest.expected, Verdict::Pass);
    }

    #[test]
    fn parses_real_manifest() {
        let text = TEXT.replace("kind = \"good\"", "kind = \"real\"");
        let manifest = parse_manifest(&text, "varint", "varint").unwrap();
        assert_eq!(manifest.kind, Kind::Real);
        assert_eq!(manifest.expected, Verdict::Pass);
        let text = format!("{text}bug = \"overflow\"\n");
        let errors = parse_manifest(&text, "varint", "varint").unwrap_err();
        assert!(errors
            .iter()
            .any(|error| error.message.contains("only valid for bad")));
    }

    #[test]
    fn real_fixtures_must_expect_pass() {
        let text = TEXT
            .replace("kind = \"good\"", "kind = \"real\"")
            .replace("expected = \"pass\"", "expected = \"diverge\"");
        let errors = parse_manifest(&text, "varint", "varint").unwrap_err();
        assert!(errors
            .iter()
            .any(|error| error.message.contains("expect pass")));
    }

    #[test]
    fn rejects_unknown_key() {
        let text = format!("{TEXT}extra = 1\n");
        let errors = parse_manifest(&text, "varint", "varint").unwrap_err();
        assert_eq!(errors[0].code, "TV0002");
    }

    #[test]
    fn rejects_wrong_schema_and_missing_key() {
        let text = TEXT.replace("schema = 1", "schema = 2");
        let errors = parse_manifest(&text, "varint", "varint").unwrap_err();
        assert!(errors.iter().any(|error| error.code == "TV0003"));
        let text = TEXT.replace("note = \"loop edges\"\n", "");
        let errors = parse_manifest(&text, "varint", "varint").unwrap_err();
        assert!(errors.iter().any(|error| error.code == "TV0003"));
    }

    #[test]
    fn requires_bug_for_bad_fixtures() {
        let text = TEXT.replace("kind = \"good\"", "kind = \"bad\"");
        let errors = parse_manifest(&text, "varint", "varint").unwrap_err();
        assert!(errors
            .iter()
            .any(|error| error.message.contains("bug is required")));
    }

    #[test]
    fn rejects_empty_classes_and_bug_on_good() {
        let text = TEXT.replace("[\"ret\", \"buf\"]", "[]");

        let errors = parse_manifest(&text, "varint", "varint").unwrap_err();
        assert!(errors
            .iter()
            .any(|error| error.code == "TV0003"
                && error.message.contains("classes must not be empty")));
        let text = format!("{TEXT}bug = \"overflow\"\n");
        let errors = parse_manifest(&text, "varint", "varint").unwrap_err();
        assert!(errors
            .iter()
            .any(|error| error.code == "TV0003" && error.message.contains("only valid for bad")));
    }

    #[test]
    fn parses_transpile_flag_on_good_and_real_but_not_bad() {
        let text = format!("{TEXT}transpile = true\n");
        let manifest = parse_manifest(&text, "varint", "varint").unwrap();
        assert!(manifest.transpile);
        let text = TEXT
            .replace("kind = \"good\"", "kind = \"bad\"")
            .replace("expected = \"pass\"", "expected = \"diverge\"")
            .replace(
                "note = \"loop edges\"",
                "bug = \"off-by-one\"\nnote = \"loop edges\"\ntranspile = true",
            );
        let errors = parse_manifest(&text, "varint", "varint").unwrap_err();
        assert!(errors.iter().any(|error| error
            .message
            .contains("transpile is only valid for good and real")));
        let text = TEXT.replace("kind = \"good\"", "kind = \"real\"").replace(
            "note = \"loop edges\"",
            "note = \"loop edges\"\ntranspile = true",
        );
        let manifest = parse_manifest(&text, "varint", "varint").unwrap();
        assert!(manifest.transpile);
    }
    #[test]
    fn parses_corpus_cases() {
        let cases = parse_corpus("-\n2a\n00ff\n", "f").unwrap();
        assert_eq!(cases, vec![vec![], vec![0x2a], vec![0, 0xff]]);
    }

    #[test]
    fn rejects_bad_corpus_lines() {
        let errors = parse_corpus("abc\n", "f").unwrap_err();
        assert_eq!(errors[0].case, Some(0));
        assert_eq!(errors[0].code, "TV0004");
        let errors = parse_corpus("zz\n", "f").unwrap_err();
        assert_eq!(errors[0].code, "TV0004");
        let errors = parse_corpus("\n", "f").unwrap_err();
        assert_eq!(errors[0].code, "TV0004");
    }

    #[test]
    fn rejects_empty_corpus() {
        let errors = parse_corpus("", "f").unwrap_err();
        assert_eq!(errors[0].code, "TV0004");
        assert_eq!(errors[0].case, None);
    }
    #[test]
    fn parses_expected_divergence_fields() {
        let text = TEXT
            .replace("kind = \"good\"", "kind = \"bad\"")
            .replace("expected = \"pass\"", "expected = \"diverge\"")
            .replace(
                "note = \"loop edges\"",
                "bug = \"overflow\"\nexpect_case = 5\nexpect_class = \"ret\"\nnote = \"loop edges\"",
            );
        let manifest = parse_manifest(&text, "varint", "varint").unwrap();
        assert_eq!(manifest.expect_case, Some(5));
        assert_eq!(manifest.expect_class, Some(Class::Ret));
        let text = text.replace("expect_class = \"ret\"", "expect_class = \"err\"");
        let manifest = parse_manifest(&text, "varint", "varint").unwrap();
        assert_eq!(manifest.expect_class, Some(Class::Err));
    }

    #[test]
    fn bad_fixtures_require_both_expectation_fields() {
        let text = TEXT
            .replace("kind = \"good\"", "kind = \"bad\"")
            .replace("expected = \"pass\"", "expected = \"diverge\"")
            .replace(
                "note = \"loop edges\"",
                "bug = \"overflow\"\nnote = \"loop edges\"",
            );
        let errors = parse_manifest(&text, "varint", "varint").unwrap_err();
        assert!(errors
            .iter()
            .any(|error| error.message.contains("expect_case is required")));
        assert!(errors
            .iter()
            .any(|error| error.message.contains("expect_class is required")));
        let text = format!("{TEXT}expect_case = 1\n");
        let errors = parse_manifest(&text, "varint", "varint").unwrap_err();
        assert!(errors
            .iter()
            .any(|error| error.message.contains("expect_case is only valid")));
    }

    #[test]
    fn rejects_corpus_line_over_request_cap() {
        let line = "2a".repeat(abi::REQUEST_CAP + 1);
        let errors = parse_corpus(&line, "f").unwrap_err();
        assert_eq!(errors[0].code, "TV0004");
        assert_eq!(errors[0].case, Some(0));
        assert!(errors[0].message.contains("4096-byte request cap"));
    }
}
