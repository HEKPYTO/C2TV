pub fn hash(data: &[u8]) -> u32 {
    const C1: u32 = 0xcc9e2d51;
    const C2: u32 = 0x1b873593;
    let mut h: u32 = 0;
    let mut chunks = data.chunks_exact(4);
    for chunk in &mut chunks {
        let mut k = u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
        k = k.wrapping_mul(C1);
        k = k.rotate_left(15);
        k = k.wrapping_mul(C2);
        h ^= k;
        h = h.rotate_left(13);
        h = h.wrapping_mul(5).wrapping_add(0xe6546b64);
    }
    let tail = chunks.remainder();
    let mut k: u32 = 0;
    if tail.len() >= 3 {
        k ^= (tail[2] as u32) << 16;
    }
    if tail.len() >= 2 {
        k ^= (tail[1] as u32) << 8;
    }
    if !tail.is_empty() {
        k ^= tail[0] as u32;
        k = k.wrapping_mul(C1);
        k = k.rotate_left(15);
        k = k.wrapping_mul(C2);
        h ^= k;
    }
    h ^= data.len() as u32;
    h ^= h >> 16;
    h = h.wrapping_mul(0x85ebca6b);
    h ^= h >> 13;
    h = h.wrapping_mul(0xc2b2ae35);
    h ^= h >> 16;
    h
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hashes_known_vectors() {
        assert_eq!(hash(b""), 0);
        assert_eq!(hash(b"a"), 0x3c2569b2);
        assert_eq!(hash(b"abc"), 0xb3dd93fa);
    }

    #[test]
    fn hashes_block_and_tail_lengths() {
        assert_eq!(hash(&[1, 2, 3, 4]), 0x3e349da5);
        assert_eq!(hash(&[1, 2]), 1690789502);
        assert_eq!(hash(&[1, 2, 3, 4, 5]), 0xa291b9c8);
        assert_eq!(hash(&[0; 16]), 0x8134cdf8);
    }

    #[test]
    fn hash_is_stable() {
        assert_eq!(hash(&[1, 2, 3, 4, 5, 6, 7, 8]), 0x0d4b1fbb);
        assert_eq!(hash(b"abc"), hash(b"abc"));
    }
}
