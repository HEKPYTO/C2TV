use std::io::{self, Read, Write};

use harness::{decode, encode};

fn read_exact(n: usize) -> Option<Vec<u8>> {
    let mut buf = vec![0u8; n];
    io::stdin().read_exact(&mut buf).ok()?;
    Some(buf)
}

fn main() {
    let hdr = match read_exact(4) {
        Some(v) => v,
        None => std::process::exit(2),
    };
    let n = u32::from_le_bytes([hdr[0], hdr[1], hdr[2], hdr[3]]) as usize;
    if n > 4096 {
        std::process::exit(2);
    }
    let input = match read_exact(n) {
        Some(v) => v,
        None => std::process::exit(2),
    };
    let (status, ret, out) = solve(&input);
    let mut resp = Vec::with_capacity(20 + out.len());
    resp.extend_from_slice(&status.to_le_bytes());
    resp.extend_from_slice(&ret.to_le_bytes());
    resp.extend_from_slice(&(out.len() as u32).to_le_bytes());
    resp.extend_from_slice(&out);
    if io::stdout().write_all(&resp).is_err() {
        std::process::exit(2);
    }
}

fn solve(input: &[u8]) -> (u64, u64, Vec<u8>) {
    if input.is_empty() {
        return (1, 0, Vec::new());
    }
    match input[0] {
        0 => {
            let body = &input[1..];
            match decode(body) {
                Some((v, used)) if used == body.len() => (0, v, encode(v)),
                _ => (1, 0, Vec::new()),
            }
        }
        1 => {
            if input.len() != 9 {
                return (1, 0, Vec::new());
            }
            let mut b = [0u8; 8];
            b.copy_from_slice(&input[1..]);
            let v = u64::from_le_bytes(b);
            (0, v, encode(v))
        }
        _ => (1, 0, Vec::new()),
    }
}
