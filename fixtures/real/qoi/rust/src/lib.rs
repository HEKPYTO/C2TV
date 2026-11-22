const MAX_PIXELS: u64 = 800;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QoiImage {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<[u8; 4]>,
}

fn be32(bytes: &[u8]) -> u32 {
    u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
}

fn hash(pixel: [u8; 4]) -> usize {
    let h = pixel[0] as u32 * 3
        + pixel[1] as u32 * 5
        + pixel[2] as u32 * 7
        + pixel[3] as u32 * 11;
    (h & 63) as usize
}

fn chan_diff(a: u8, b: u8) -> i32 {
    let d = a as i32 - b as i32;
    if d > 127 {
        d - 256
    } else if d < -128 {
        d + 256
    } else {
        d
    }
}

pub fn qoi_decode(input: &[u8]) -> Option<QoiImage> {
    if input.len() < 22 || &input[..4] != b"qoif" {
        return None;
    }
    let width = be32(&input[4..8]);
    let height = be32(&input[8..12]);
    let channels = input[12];
    let colorspace = input[13];
    let count = u64::from(width) * u64::from(height);
    if count == 0
        || count > MAX_PIXELS
        || !(3..=4).contains(&channels)
        || colorspace > 1
    {
        return None;
    }
    let chunks_end = input.len() - 8;
    if chunks_end < 14 || &input[chunks_end..] != &[0, 0, 0, 0, 0, 0, 0, 1] {
        return None;
    }
    let mut index = [[0u8; 4]; 64];
    let mut px = [0u8, 0, 0, 255];
    let mut p = 14usize;
    let mut run = 0u32;
    let mut pixels = Vec::with_capacity(count as usize);
    for _ in 0..count {
        if run > 0 {
            run -= 1;
        } else {
            if p >= chunks_end {
                return None;
            }
            let b1 = input[p];
            p += 1;
            if b1 == 0xfe {
                if p + 3 > chunks_end {
                    return None;
                }
                px[0] = input[p];
                px[1] = input[p + 1];
                px[2] = input[p + 2];
                p += 3;
            } else if b1 == 0xff {
                if p + 4 > chunks_end {
                    return None;
                }
                px[0] = input[p];
                px[1] = input[p + 1];
                px[2] = input[p + 2];
                px[3] = input[p + 3];
                p += 4;
            } else {
                match b1 & 0xc0 {
                    0x00 => px = index[(b1 & 0x3f) as usize],
                    0x40 => {
                        px[0] = px[0].wrapping_add((((b1 >> 4) & 3) as i32 - 2) as u8);
                        px[1] = px[1].wrapping_add((((b1 >> 2) & 3) as i32 - 2) as u8);
                        px[2] = px[2].wrapping_add(((b1 & 3) as i32 - 2) as u8);
                    }
                    0x80 => {
                        if p >= chunks_end {
                            return None;
                        }
                        let b2 = input[p];
                        p += 1;
                        let vg = (b1 & 0x3f) as i32 - 32;
                        px[0] = px[0].wrapping_add((vg - 8 + ((b2 >> 4) & 15) as i32) as u8);
                        px[1] = px[1].wrapping_add(vg as u8);
                        px[2] = px[2].wrapping_add((vg - 8 + (b2 & 15) as i32) as u8);
                    }
                    0xc0 => run = (b1 & 0x3f) as u32,
                    _ => return None,
                }
            }
            index[hash(px)] = px;
        }
        pixels.push(px);
    }
    Some(QoiImage {
        width,
        height,
        pixels,
    })
}

pub fn qoi_encode(image: &QoiImage) -> Option<Vec<u8>> {
    let count = u64::from(image.width) * u64::from(image.height);
    if count != image.pixels.len() as u64 || count == 0 || count > MAX_PIXELS {
        return None;
    }
    let mut out = Vec::new();
    out.extend_from_slice(b"qoif");
    out.extend_from_slice(&image.width.to_be_bytes());
    out.extend_from_slice(&image.height.to_be_bytes());
    out.push(4);
    out.push(0);
    let mut index = [[0u8; 4]; 64];
    let mut prev = [0u8, 0, 0, 255];
    let mut run = 0u32;
    for (i, &px) in image.pixels.iter().enumerate() {
        if px == prev {
            run += 1;
            if run == 62 || i + 1 == image.pixels.len() {
                out.push(0xc0 | (run as u8 - 1));
                run = 0;
            }
        } else {
            if run > 0 {
                out.push(0xc0 | (run as u8 - 1));
                run = 0;
            }
            let idx = hash(px);
            if index[idx] == px {
                out.push(idx as u8);
                index[idx] = px;
            } else {
                index[idx] = px;
                if px[3] == prev[3] {
                    let dr = chan_diff(px[0], prev[0]);
                    let dg = chan_diff(px[1], prev[1]);
                    let db = chan_diff(px[2], prev[2]);
                    if (-2..=1).contains(&dr) && (-2..=1).contains(&dg) && (-2..=1).contains(&db) {
                        out.push(
                            0x40 | ((dr + 2) as u8) << 4 | ((dg + 2) as u8) << 2 | (db + 2) as u8,
                        );
                    } else {
                        let vg = dg;
                        let drdg = dr - vg;
                        let dbdg = db - vg;
                        if (-32..=31).contains(&vg)
                            && (-8..=7).contains(&drdg)
                            && (-8..=7).contains(&dbdg)
                        {
                            out.push(0x80 | (vg + 32) as u8);
                            out.push(((drdg + 8) as u8) << 4 | (dbdg + 8) as u8);
                        } else {
                            out.extend_from_slice(&[0xfe, px[0], px[1], px[2]]);
                        }
                    }
                } else {
                    out.extend_from_slice(&[0xff, px[0], px[1], px[2], px[3]]);
                }
            }
        }
        prev = px;
    }
    out.extend_from_slice(&[0, 0, 0, 0, 0, 0, 0, 1]);
    Some(out)
}

pub fn qoi_roundtrip(input: &[u8]) -> Option<Vec<u8>> {
    qoi_encode(&qoi_decode(input)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bytes(hex: &str) -> Vec<u8> {
        (0..hex.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
            .collect()
    }

    fn header(w: u32, h: u32) -> Vec<u8> {
        let mut v = Vec::new();
        v.extend_from_slice(b"qoif");
        v.extend_from_slice(&w.to_be_bytes());
        v.extend_from_slice(&h.to_be_bytes());
        v.extend_from_slice(&[4, 0]);
        v
    }

    fn stream(w: u32, h: u32, chunks: &[u8]) -> Vec<u8> {
        let mut v = header(w, h);
        v.extend_from_slice(chunks);
        v.extend_from_slice(&[0, 0, 0, 0, 0, 0, 0, 1]);
        v
    }

    #[test]
    fn decodes_rgb_and_rgba_chunks() {
        let rgb = qoi_decode(&stream(1, 1, &[0xfe, 0x10, 0x20, 0x30])).unwrap();
        assert_eq!(rgb.pixels, vec![[0x10, 0x20, 0x30, 255]]);
        let rgba = qoi_decode(&stream(1, 1, &[0xff, 0x10, 0x20, 0x30, 0x40])).unwrap();
        assert_eq!(rgba.pixels, vec![[0x10, 0x20, 0x30, 0x40]]);
    }

    #[test]
    fn decodes_run_and_index_chunks() {
        let run = qoi_decode(&stream(3, 1, &[0xfe, 0x11, 0x22, 0x33, 0xc2])).unwrap();
        assert_eq!(run.pixels, vec![[0x11, 0x22, 0x33, 255]; 3]);
        let index = qoi_decode(&stream(2, 1, &[0xff, 0x10, 0x20, 0x30, 0x40, 0x20])).unwrap();
        assert_eq!(
            index.pixels,
            vec![[0x10, 0x20, 0x30, 0x40], [0x10, 0x20, 0x30, 0x40]]
        );
    }

    #[test]
    fn decodes_and_reencodes_luma() {
        let input = bytes("716f696600000002000000010400fe0a141ea5560000000000000001");
        let image = qoi_decode(&input).unwrap();
        assert_eq!(
            image.pixels,
            vec![[10, 20, 30, 255], [12, 25, 33, 255]]
        );
        let encoded = qoi_encode(&image).unwrap();
        assert_eq!(qoi_decode(&encoded).unwrap(), image);
        assert_eq!(encoded, input);
    }

    #[test]
    fn roundtrips_one_by_one_and_two_by_two() {
        let one = stream(1, 1, &[0xff, 1, 2, 3, 4]);
        assert_eq!(qoi_roundtrip(&one).unwrap(), one);
        let two = stream(
            2,
            2,
            &[0xfe, 10, 20, 30, 0x7a, 0xfe, 200, 100, 50, 0xff, 200, 100, 50, 100],
        );
        let image = qoi_decode(&two).unwrap();
        assert_eq!(
            image.pixels,
            vec![
                [10, 20, 30, 255],
                [11, 20, 30, 255],
                [200, 100, 50, 255],
                [200, 100, 50, 100],
            ]
        );
        assert_eq!(qoi_roundtrip(&two).unwrap(), two);
    }

    #[test]
    fn rejects_bad_streams() {
        let mut bad = bytes("716f696600000001000000010400fe1020300000000000000001");
        bad[0] = 0;
        assert!(qoi_decode(&bad).is_none());
        assert!(qoi_decode(&[0u8; 8]).is_none());
        let over_cap = bytes("716f696600000003210000000104000000000000000001");
        assert!(qoi_decode(&over_cap).is_none());
    }
}
