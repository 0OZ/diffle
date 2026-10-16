use super::*;

async fn delta_of(content: &[u8], reversed: bool) -> Delta {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("f");
    std::fs::write(&file, content).unwrap();
    untracked_delta(&file, reversed).await
}

fn added(additions: u32) -> Delta {
    Delta::Text {
        additions,
        deletions: 0,
    }
}

#[tokio::test]
async fn counts_lines_with_or_without_a_final_newline() {
    assert_eq!(delta_of(b"", false).await, added(0));
    assert_eq!(delta_of(b"one\n", false).await, added(1));
    assert_eq!(delta_of(b"one\ntwo", false).await, added(2));
    assert_eq!(delta_of(b"\n\n", false).await, added(2));
}

#[tokio::test]
async fn seen_from_the_other_side_the_lines_are_deletions() {
    assert_eq!(
        delta_of(b"a\nb\n", true).await,
        Delta::Text {
            additions: 0,
            deletions: 2
        }
    );
}

#[tokio::test]
async fn a_nul_in_the_first_bytes_makes_it_binary_but_a_late_one_does_not() {
    assert_eq!(delta_of(b"ab\0cd", false).await, Delta::Binary);
    let mut late = vec![b'x'; 9000];
    late.push(0);
    assert_eq!(delta_of(&late, false).await, added(1));
}

#[tokio::test]
async fn counts_across_chunk_boundaries() {
    let content = "line\n".repeat(40_000);
    assert_eq!(delta_of(content.as_bytes(), false).await, added(40_000));
}

#[tokio::test]
async fn a_missing_file_counts_as_empty() {
    let dir = tempfile::tempdir().unwrap();
    assert_eq!(
        untracked_delta(&dir.path().join("nope"), false).await,
        added(0)
    );
}

#[cfg(unix)]
#[tokio::test]
async fn a_symlink_counts_its_target_text() {
    let dir = tempfile::tempdir().unwrap();
    let link = dir.path().join("link");
    std::os::unix::fs::symlink("some/target", &link).unwrap();
    assert_eq!(untracked_delta(&link, false).await, added(1));
}
