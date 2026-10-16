use super::*;

#[test]
fn a_nul_within_the_sniffed_head_means_binary() {
    assert!(is_binary(b"\0"));
    assert!(is_binary(b"text\0more"));
    let mut edge = vec![b'a'; BINARY_SNIFF_BYTES];
    edge[BINARY_SNIFF_BYTES - 1] = 0;
    assert!(is_binary(&edge));
}

#[test]
fn text_and_a_nul_past_the_head_are_not_binary() {
    assert!(!is_binary(b""));
    assert!(!is_binary("héllo\nwörld\n".as_bytes()));
    let mut late = vec![b'a'; BINARY_SNIFF_BYTES + 1];
    late[BINARY_SNIFF_BYTES] = 0;
    assert!(!is_binary(&late));
}
