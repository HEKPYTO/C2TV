use std::io::{self, Read, Write};

use harness::merge;

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
    let err = (1, 0, Vec::new());
    if input.len() < 4 {
        return err;
    }
    let na = u32::from_le_bytes(input[0..4].try_into().unwrap()) as usize;
    if na > 8 || 4 + na * 4 > input.len() {
        return err;
    }
    let mut off = 4;
    let mut a = Vec::with_capacity(na);
    for _ in 0..na {
        a.push(u32::from_le_bytes(input[off..off + 4].try_into().unwrap()));
        off += 4;
    }
    if off + 4 > input.len() {
        return err;
    }
    let nb = u32::from_le_bytes(input[off..off + 4].try_into().unwrap()) as usize;
    off += 4;
    if nb > 8 || off + nb * 4 != input.len() {
        return err;
    }
    let mut b = Vec::with_capacity(nb);
    for _ in 0..nb {
        b.push(u32::from_le_bytes(input[off..off + 4].try_into().unwrap()));
        off += 4;
    }
    let merged = match merge(&a, &b) {
        Some(v) => v,
        None => return err,
    };
    let mut out = Vec::with_capacity(merged.len() * 4);
    for v in merged {
        out.extend_from_slice(&v.to_le_bytes());
    }
    (0, (a.len() + b.len()) as u64, out)
}
