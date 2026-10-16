use super::*;

#[test]
fn keeps_the_lines_it_was_given_and_contains_both_ends() {
    let span = LineSpan::new(3, 5).unwrap();
    assert_eq!((span.start(), span.end()), (3, 5));
    assert!(!span.contains(2));
    assert!(span.contains(3) && span.contains(4) && span.contains(5));
    assert!(!span.contains(6));
    assert!(LineSpan::new(7, 7).unwrap().contains(7));
}

#[test]
fn refuses_a_zeroth_line_and_a_backwards_span() {
    assert_eq!(LineSpan::new(0, 3), Err(LineSpanError::BeforeFirstLine));
    assert_eq!(LineSpan::new(5, 4), Err(LineSpanError::EndsBeforeStart));
}
