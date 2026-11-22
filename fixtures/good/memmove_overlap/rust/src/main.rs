use std::io::{self, Read, Write};

use harness::copy_within;

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
    if input.len() < 12 {
        return (1, 0, Vec::new());
    }
    let src = u32::from_le_bytes([input[0], input[1], input[2], input[3]]) as usize;
    let dst = u32::from_le_bytes([input[4], input[5], input[6], input[7]]) as usize;
    let len = u32::from_le_bytes([input[8], input[9], input[10], input[11]]) as usize;
    let mut buf = input[12..].to_vec();
    if !copy_within(&mut buf, src, dst, len) {
        return (1, 0, Vec::new());
    }
    (0, 0, buf)
}
