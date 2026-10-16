use super::*;

fn watching(source: notify::Error) -> WatchError {
    WatchError::Watch {
        path: PathBuf::from("/repo/src"),
        source,
    }
}

#[test]
fn running_out_of_watches_says_how_to_get_more() {
    let error = watching(notify::Error::new(notify::ErrorKind::MaxFilesWatch));
    let text = error.explained();
    assert!(text.starts_with("cannot watch /repo/src: "), "{text}");
    assert!(
        text.ends_with("(raise fs.inotify.max_user_watches, or start with --no-watch)"),
        "{text}"
    );
}

#[test]
fn other_failures_are_reported_as_they_are() {
    let error = watching(notify::Error::path_not_found());
    assert_eq!(error.explained(), error.to_string());
}
