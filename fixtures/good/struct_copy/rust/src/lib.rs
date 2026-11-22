#[repr(C)]
pub struct Rec {
    pub a: u32,
    pub b: u8,
    pub c: u16,
    pub d: u8,
}

pub fn canonical(rec: &Rec) -> [u8; 12] {
    let mut out = [0u8; 12];
    out[0..4].copy_from_slice(&rec.a.to_le_bytes());
    out[4] = rec.b;
    out[6..8].copy_from_slice(&rec.c.to_le_bytes());
    out[8] = rec.d;
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_matches_expected_offsets() {
        assert_eq!(std::mem::size_of::<Rec>(), 12);
        let rec = Rec { a: 1, b: 2, c: 3, d: 4 };
        let addr = &rec as *const Rec as usize;
        assert_eq!(&rec.b as *const u8 as usize - addr, 4);
        assert_eq!(&rec.c as *const u16 as usize - addr, 6);
        assert_eq!(&rec.d as *const u8 as usize - addr, 8);
    }

    #[test]
    fn emits_canonical_bytes() {
        let rec = Rec { a: 0x04030201, b: 5, c: 0x0706, d: 9 };
        assert_eq!(canonical(&rec), [1, 2, 3, 4, 5, 0, 6, 7, 9, 0, 0, 0]);
    }

    #[test]
    fn zeroes_padding_for_max_fields() {
        let rec = Rec { a: u32::MAX, b: 255, c: u16::MAX, d: 255 };
        assert_eq!(canonical(&rec), [255, 255, 255, 255, 255, 0, 255, 255, 255, 0, 0, 0]);
    }
}
