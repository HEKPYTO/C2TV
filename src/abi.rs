use crate::compare::Observation;
use crate::diag::{self, Diagnostic, Level};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProtocolError {
    ShortHeader,
    ShortBody,
    Trailing,
    BadStatus,
}

impl ProtocolError {
    pub fn diagnostic(self, fixture: &str, case: usize, level: Level) -> Diagnostic {
        match self {
            ProtocolError::ShortHeader | ProtocolError::ShortBody => Diagnostic::new(
                diag::SHORT_READ,
                fixture,
                Some(case),
                Some(level),
                "response frame shorter than declared length",
            ),
            ProtocolError::Trailing => Diagnostic::new(
                diag::TRAILING,
                fixture,
                Some(case),
                Some(level),
                "response frame has trailing bytes",
            ),
            ProtocolError::BadStatus => Diagnostic::new(
                diag::BAD_STATUS,
                fixture,
                Some(case),
                Some(level),
                "response status is neither 0 nor 1",
            ),
        }
    }
}

pub fn encode_request(payload: &[u8]) -> Option<Vec<u8>> {
    let len = u32::try_from(payload.len()).ok()?;
    let mut frame = Vec::with_capacity(4 + payload.len());
    frame.extend_from_slice(&len.to_le_bytes());
    frame.extend_from_slice(payload);
    Some(frame)
}

pub fn parse_response(bytes: &[u8]) -> Result<Observation, ProtocolError> {
    if bytes.len() < 20 {
        return Err(ProtocolError::ShortHeader);
    }
    let status = u64::from_le_bytes(bytes[0..8].try_into().unwrap());
    let ret = u64::from_le_bytes(bytes[8..16].try_into().unwrap());
    let out_len = u32::from_le_bytes(bytes[16..20].try_into().unwrap()) as usize;
    let end = 20usize
        .checked_add(out_len)
        .ok_or(ProtocolError::ShortBody)?;
    if bytes.len() < end {
        return Err(ProtocolError::ShortBody);
    }
    if bytes.len() > end {
        return Err(ProtocolError::Trailing);
    }
    if status > 1 {
        return Err(ProtocolError::BadStatus);
    }
    Ok(Observation {
        status,
        ret,
        out: bytes[20..end].to_vec(),
    })
}

#[cfg(test)]
pub fn response_bytes(observation: &Observation) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(20 + observation.out.len());
    bytes.extend_from_slice(&observation.status.to_le_bytes());
    bytes.extend_from_slice(&observation.ret.to_le_bytes());
    bytes.extend_from_slice(&(observation.out.len() as u32).to_le_bytes());
    bytes.extend_from_slice(&observation.out);
    bytes
}

#[cfg(test)]
mod tests {
    use super::*;

    fn observation(status: u64, ret: u64, out: &[u8]) -> Observation {
        Observation {
            status,
            ret,
            out: out.to_vec(),
        }
    }

    #[test]
    fn encodes_request_frame() {
        assert_eq!(encode_request(&[]), Some(vec![0, 0, 0, 0]));
        assert_eq!(encode_request(&[0x2a]), Some(vec![1, 0, 0, 0, 0x2a]));
        assert_eq!(encode_request(&[1, 2]), Some(vec![2, 0, 0, 0, 1, 2]));
    }

    #[test]
    fn parses_exact_response() {
        let bytes = response_bytes(&observation(1, 258, &[7, 8]));
        let parsed = parse_response(&bytes).unwrap();
        assert_eq!(parsed.status, 1);
        assert_eq!(parsed.ret, 258);
        assert_eq!(parsed.out, vec![7, 8]);
    }

    #[test]
    fn rejects_short_header() {
        assert_eq!(parse_response(&[0; 19]), Err(ProtocolError::ShortHeader));
        assert_eq!(parse_response(&[]), Err(ProtocolError::ShortHeader));
    }

    #[test]
    fn rejects_short_body() {
        let mut bytes = vec![0u8; 20];
        bytes[16..20].copy_from_slice(&2u32.to_le_bytes());
        bytes.push(1);
        assert_eq!(parse_response(&bytes), Err(ProtocolError::ShortBody));
    }

    #[test]
    fn rejects_trailing_bytes() {
        let mut bytes = response_bytes(&observation(0, 0, &[]));
        bytes.push(0);
        assert_eq!(parse_response(&bytes), Err(ProtocolError::Trailing));
    }

    #[test]
    fn rejects_invalid_status_and_overflow() {
        let mut bytes = response_bytes(&observation(2, 0, &[]));
        assert_eq!(parse_response(&bytes), Err(ProtocolError::BadStatus));
        bytes[16..20].copy_from_slice(&u32::MAX.to_le_bytes());
        assert_eq!(parse_response(&bytes), Err(ProtocolError::ShortBody));
    }

    #[test]
    fn bad_status_has_own_diagnostic() {
        let diagnostic = ProtocolError::BadStatus.diagnostic("f", 3, Level::O2);
        assert_eq!(diagnostic.code, "TV0015");
        assert_eq!(diagnostic.case, Some(3));
        assert_eq!(diagnostic.level, Some(Level::O2));
    }
}
