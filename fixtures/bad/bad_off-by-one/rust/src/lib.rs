pub fn echo(data: &[u8]) -> Vec<u8> {
    data.to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_echo() {
        assert!(echo(&[]).is_empty());
    }

    #[test]
    fn echo_preserves_every_byte() {
        assert_eq!(echo(&[0x41]), vec![0x41]);
        assert_eq!(echo(&[0x41, 0x42, 0x43]), vec![0x41, 0x42, 0x43]);
        assert_eq!(echo(&[0, 255, 0]), vec![0, 255, 0]);
    }
}
