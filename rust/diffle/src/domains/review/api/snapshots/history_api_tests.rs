use super::*;

#[test]
fn an_offset_is_digits_that_fit() {
    assert_eq!(offset("0").unwrap(), 0);
    assert_eq!(offset("12").unwrap(), 12);
    for bad in ["", "-1", "1.5", "x", " 3", "99999999999"] {
        assert!(offset(bad).is_err(), "{bad:?}");
    }
}
