use crate::diag::{self, Diagnostic, Level};
use crate::manifest::Class;
use serde::Serialize;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Observation {
    pub status: u64,
    pub ret: u64,
    pub out: Vec<u8>,
}

pub const FIELDS: [&str; 3] = ["buf", "ret", "status"];

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct FirstDiff {
    pub field: &'static str,
    pub offset: usize,
    pub c: u8,
    pub r: u8,
}

impl FirstDiff {
    pub fn message(self) -> String {
        format!(
            "first differing byte in {} at offset {}: c=0x{:02x} r=0x{:02x}",
            self.field, self.offset, self.c, self.r
        )
    }
}

pub fn covers(classes: &[Class], field: &str) -> bool {
    match field {
        "buf" => classes.contains(&Class::Buf),
        "ret" => classes.contains(&Class::Ret),
        "status" => classes.contains(&Class::Err),
        _ => false,
    }
}

pub fn not_compared(classes: &[Class]) -> Vec<&'static str> {
    FIELDS
        .iter()
        .copied()
        .filter(|field| !covers(classes, field))
        .collect()
}

fn field_bytes(observation: &Observation, field: &str) -> Vec<u8> {
    match field {
        "status" => observation.status.to_le_bytes().to_vec(),
        "ret" => observation.ret.to_le_bytes().to_vec(),
        "buf" => {
            let mut bytes = (observation.out.len() as u32).to_le_bytes().to_vec();
            bytes.extend_from_slice(&observation.out);
            bytes
        }
        _ => Vec::new(),
    }
}

fn first_difference(left: &[u8], right: &[u8]) -> Option<usize> {
    let common = left.len().min(right.len());
    if let Some(offset) = left[..common]
        .iter()
        .zip(right[..common].iter())
        .position(|(a, b)| a != b)
    {
        return Some(offset);
    }
    (left.len() != right.len()).then_some(common)
}

pub fn compare(
    fixture: &str,
    case: usize,
    level: Level,
    c: &Observation,
    r: &Observation,
    classes: &[Class],
) -> (Option<FirstDiff>, Vec<Diagnostic>) {
    let mut first = None;
    for field in FIELDS {
        if !covers(classes, field) {
            continue;
        }
        let left = field_bytes(c, field);
        let right = field_bytes(r, field);
        if let Some(offset) = first_difference(&left, &right) {
            let diff = FirstDiff {
                field,
                offset,
                c: left.get(offset).copied().unwrap_or(0),
                r: right.get(offset).copied().unwrap_or(0),
            };
            if first.is_none() {
                first = Some(diff);
            }
        }
    }
    let diagnostics = match first {
        Some(diff) => vec![Diagnostic::new(
            diag::DIVERGENCE,
            fixture,
            Some(case),
            Some(level),
            diff.message(),
        )],
        None => Vec::new(),
    };
    (first, diagnostics)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn response(status: u64, ret: u64, out: &[u8]) -> Observation {
        Observation {
            status,
            ret,
            out: out.to_vec(),
        }
    }

    #[test]
    fn masks_undeclared_fields() {
        let c = response(0, 1, &[9]);
        let r = response(1, 1, &[8]);
        let (diff, diagnostics) = compare("f", 0, Level::O0, &c, &r, &[Class::Ret]);
        assert_eq!(diff, None);
        assert!(diagnostics.is_empty());
        assert_eq!(not_compared(&[Class::Ret]), vec!["buf", "status"]);
    }

    #[test]
    fn reports_first_differing_byte() {
        let c = response(0, 0x1234, &[1, 2, 3]);
        let r = response(0, 0x0434, &[1, 2, 3]);
        let (diff, diagnostics) = compare("f", 4, Level::O2, &c, &r, &[Class::Buf, Class::Ret]);
        let diff = diff.unwrap();
        assert_eq!(diff.field, "ret");
        assert_eq!(diff.offset, 1);
        assert_eq!(diff.c, 0x12);
        assert_eq!(diff.r, 0x04);
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "TV0008");
    }

    #[test]
    fn detects_buffer_length_change() {
        let c = response(0, 0, &[1]);
        let r = response(0, 0, &[1, 2]);
        let (diff, _) = compare("f", 0, Level::O0, &c, &r, &[Class::Buf]);
        let diff = diff.unwrap();
        assert_eq!(diff.field, "buf");
        assert_eq!(diff.offset, 0);
        assert_eq!(diff.c, 1);
        assert_eq!(diff.r, 2);
    }

    #[test]
    fn compares_status_only_when_declared() {
        let c = response(0, 0, &[]);
        let r = response(1, 0, &[]);
        assert_eq!(compare("f", 0, Level::O0, &c, &r, &[Class::Ret]).0, None);
        let (diff, _) = compare("f", 0, Level::O0, &c, &r, &[Class::Err]);
        assert_eq!(diff.unwrap().field, "status");
    }
}
