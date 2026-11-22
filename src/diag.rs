use serde::Serialize;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Error,
    Info,
    Warning,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ExitClass {
    Ok,
    Fail,
    ToolError,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Level {
    O0,
    O2,
}

impl Level {
    pub fn label(self) -> &'static str {
        match self {
            Level::O0 => "o0",
            Level::O2 => "o2",
        }
    }

    pub fn all() -> [Level; 2] {
        [Level::O0, Level::O2]
    }
}

#[derive(Clone, Copy, Debug)]
pub struct CodeInfo {
    pub code: &'static str,
    pub name: &'static str,
    pub severity: Severity,
    pub exit_class: ExitClass,
}

pub const MANIFEST_PARSE: CodeInfo = CodeInfo {
    code: "TV0001",
    name: "manifest_parse",
    severity: Severity::Error,
    exit_class: ExitClass::ToolError,
};
pub const UNKNOWN_KEY: CodeInfo = CodeInfo {
    code: "TV0002",
    name: "unknown_key",
    severity: Severity::Error,
    exit_class: ExitClass::ToolError,
};
pub const MANIFEST_VALUE: CodeInfo = CodeInfo {
    code: "TV0003",
    name: "manifest_value",
    severity: Severity::Error,
    exit_class: ExitClass::ToolError,
};
pub const CORPUS_HEX: CodeInfo = CodeInfo {
    code: "TV0004",
    name: "corpus_hex",
    severity: Severity::Error,
    exit_class: ExitClass::ToolError,
};
pub const SPAWN: CodeInfo = CodeInfo {
    code: "TV0005",
    name: "spawn",
    severity: Severity::Error,
    exit_class: ExitClass::ToolError,
};
pub const TIMEOUT: CodeInfo = CodeInfo {
    code: "TV0006",
    name: "timeout",
    severity: Severity::Error,
    exit_class: ExitClass::ToolError,
};
pub const NONZERO_EXIT: CodeInfo = CodeInfo {
    code: "TV0007",
    name: "nonzero_exit",
    severity: Severity::Error,
    exit_class: ExitClass::ToolError,
};
pub const DIVERGENCE: CodeInfo = CodeInfo {
    code: "TV0008",
    name: "divergence",
    severity: Severity::Error,
    exit_class: ExitClass::Fail,
};
pub const UB: CodeInfo = CodeInfo {
    code: "TV0009",
    name: "ub",
    severity: Severity::Warning,
    exit_class: ExitClass::Ok,
};
pub const SHORT_READ: CodeInfo = CodeInfo {
    code: "TV0010",
    name: "short_read",
    severity: Severity::Error,
    exit_class: ExitClass::ToolError,
};
pub const DIGEST: CodeInfo = CodeInfo {
    code: "TV0011",
    name: "digest_mismatch",
    severity: Severity::Error,
    exit_class: ExitClass::ToolError,
};
pub const TRAILING: CodeInfo = CodeInfo {
    code: "TV0012",
    name: "trailing_bytes",
    severity: Severity::Error,
    exit_class: ExitClass::ToolError,
};
pub const DETERMINISM: CodeInfo = CodeInfo {
    code: "TV0013",
    name: "determinism",
    severity: Severity::Error,
    exit_class: ExitClass::Fail,
};
pub const CAPABILITY: CodeInfo = CodeInfo {
    code: "TV0014",
    name: "capability_not_available",
    severity: Severity::Info,
    exit_class: ExitClass::Ok,
};
pub const BAD_STATUS: CodeInfo = CodeInfo {
    code: "TV0015",
    name: "bad_status",
    severity: Severity::Error,
    exit_class: ExitClass::ToolError,
};

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub struct Diagnostic {
    pub code: &'static str,
    pub name: &'static str,
    pub severity: Severity,
    pub fixture: String,
    pub case: Option<usize>,
    pub level: Option<Level>,
    pub message: String,
    pub exit_class: ExitClass,
}

impl Diagnostic {
    pub fn new(
        info: CodeInfo,
        fixture: &str,
        case: Option<usize>,
        level: Option<Level>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            code: info.code,
            name: info.name,
            severity: info.severity,
            fixture: fixture.to_string(),
            case,
            level,
            message: message.into(),
            exit_class: info.exit_class,
        }
    }
}

pub fn exit_code(diagnostics: &[Diagnostic]) -> i32 {
    if diagnostics
        .iter()
        .any(|d| d.exit_class == ExitClass::ToolError)
    {
        2
    } else if diagnostics.iter().any(|d| d.exit_class == ExitClass::Fail) {
        1
    } else {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(info: CodeInfo) -> Diagnostic {
        Diagnostic::new(info, "f", None, None, "m")
    }

    #[test]
    fn maps_exit_classes() {
        assert_eq!(exit_code(&[]), 0);
        assert_eq!(exit_code(&[d(UB)]), 0);
        assert_eq!(exit_code(&[d(DIVERGENCE)]), 1);
        assert_eq!(exit_code(&[d(DETERMINISM)]), 1);
        assert_eq!(exit_code(&[d(SPAWN)]), 2);
        assert_eq!(exit_code(&[d(UB), d(DIVERGENCE), d(SPAWN)]), 2);
    }

    #[test]
    fn codes_are_unique_and_contiguous() {
        let codes = [
            MANIFEST_PARSE,
            UNKNOWN_KEY,
            MANIFEST_VALUE,
            CORPUS_HEX,
            SPAWN,
            TIMEOUT,
            NONZERO_EXIT,
            DIVERGENCE,
            UB,
            SHORT_READ,
            DIGEST,
            TRAILING,
            DETERMINISM,
            CAPABILITY,
            BAD_STATUS,
        ];
        for (index, info) in codes.iter().enumerate() {
            let expected = format!("TV{:04}", index + 1);
            assert_eq!(info.code, expected);
        }
    }
}
