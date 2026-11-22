use harness::parse_sat;

#[test]
fn parses_plain_and_signed() {
    assert_eq!(parse_sat(b"0"), Some(0));
    assert_eq!(parse_sat(b"-0"), Some(0));
    assert_eq!(parse_sat(b"+42"), Some(42));
    assert_eq!(parse_sat(b"-7"), Some(-7));
}

#[test]
fn saturates_both_directions() {
    assert_eq!(parse_sat(b"2147483647"), Some(2147483647));
    assert_eq!(parse_sat(b"2147483648"), Some(2147483647));
    assert_eq!(parse_sat(b"-2147483648"), Some(-2147483648));
    assert_eq!(parse_sat(b"-2147483649"), Some(-2147483648));
}

#[test]
fn rejects_invalid_payloads() {
    assert_eq!(parse_sat(b"abc"), None);
    assert_eq!(parse_sat(b"12x"), None);
    assert_eq!(parse_sat(b""), None);
}
