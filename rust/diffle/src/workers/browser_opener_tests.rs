use super::*;

#[test]
fn the_url_is_the_last_argument_of_the_platforms_opener() {
    let command = opener("http://localhost:4966/");
    let args: Vec<_> = command.get_args().collect();
    assert_eq!(
        args.last().unwrap().to_str(),
        Some("http://localhost:4966/")
    );
    assert!(!command.get_program().is_empty());
}
