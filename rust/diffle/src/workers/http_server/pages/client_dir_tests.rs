use super::*;

#[test]
fn a_directory_with_an_index_is_a_client_build() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("index.html"), "<html>").unwrap();
    let client = ClientDir::locate(Some(dir.path())).unwrap();
    assert_eq!(client.path(), dir.path());
    assert_eq!(client.index(), dir.path().join("index.html"));
}

#[test]
fn an_explicit_directory_without_a_build_is_an_error_naming_it() {
    let dir = tempfile::tempdir().unwrap();
    let error = ClientDir::locate(Some(dir.path())).unwrap_err();
    assert!(matches!(&error, ServerError::NoClientBuild { dir: named } if named == dir.path()));
    assert!(error.to_string().contains("npm run build"));
}
