use std::io::{self, Read, Write};

use harness::maxima;

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
    if input.len() < 8 {
        return err;
    }
    let k = u32::from_le_bytes(input[0..4].try_into().unwrap()) as usize;
    if k == 0 || k > 8 {
        return err;
    }
    let count = u32::from_le_bytes(input[4..8].try_into().unwrap()) as usize;
    if count > 16 || 8 + count * 4 != input.len() {
        return err;
    }
    let mut data = Vec::with_capacity(count);
    let mut off = 8;
    for _ in 0..count {
        data.push(u32::from_le_bytes(input[off..off + 4].try_into().unwrap()));
        off += 4;
    }
    let result = maxima(&data, k);
    let mut out = Vec::with_capacity(result.len() * 4);
    for v in result {
        out.extend_from_slice(&v.to_le_bytes());
    }
    let ret = out.len() as u64 / 4;
    (0, ret, out)
}
