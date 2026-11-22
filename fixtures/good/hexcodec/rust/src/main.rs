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
    let err = (1, 0, Vec::new());
    let mode = match input.first() {
        Some(m) => *m,
        None => return err,
    };
    let body = &input[1..];
    if mode == 0 {
        let out = encode(body);
        let ret = out.len() as u64;
        (0, ret, out)
    } else if mode == 1 {
        match decode(body) {
            Some(out) => {
                let ret = out.len() as u64;
                (0, ret, out)
            }
            None => err,
        }
    } else {
        err
    }
}
