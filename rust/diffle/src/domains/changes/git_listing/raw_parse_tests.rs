use super::*;

const A: &str = "1111111111111111111111111111111111111111";
const B: &str = "2222222222222222222222222222222222222222";
const ZEROS: &str = "0000000000000000000000000000000000000000";

fn records(items: &[&str]) -> Vec<String> {
    items.iter().map(|item| (*item).to_owned()).collect()
}

fn parse(items: &[&str]) -> Vec<ChangedFile> {
    parse_raw_numstat(&records(items)).unwrap()
}

#[test]
fn reads_edits_additions_and_deletions_with_their_blobs_and_counts() {
    let files = parse(&[
        &format!(":100644 100644 {A} {B} M"),
        "src/edit.rs",
        &format!(":000000 100644 {ZEROS} {B} A"),
        "new.rs",
        &format!(":100644 000000 {A} {ZEROS} D"),
        "gone.rs",
        "3\t1\tsrc/edit.rs",
        "5\t0\tnew.rs",
        "0\t7\tgone.rs",
    ]);
    assert_eq!(files.len(), 3);
    let [edit, added, deleted] = &files[..] else {
        panic!("three files");
    };
    assert_eq!(edit.kind.code(), 'M');
    assert_eq!(
        edit.delta,
        Delta::Text {
            additions: 3,
            deletions: 1
        }
    );
    assert_eq!(edit.blob.as_ref().map(ObjectId::as_str), Some(B));
    assert_eq!(edit.old_blob.as_ref().map(ObjectId::as_str), Some(A));
    assert_eq!((added.kind.code(), added.old_blob.clone()), ('A', None));
    assert_eq!(deleted.blob, None);
    assert_eq!(
        deleted.delta,
        Delta::Text {
            additions: 0,
            deletions: 7
        }
    );
}

#[test]
fn a_rename_names_its_source_and_takes_the_counts_from_the_three_part_record() {
    let files = parse(&[
        &format!(":100644 100644 {A} {B} R091"),
        "old name.rs",
        "new name.rs",
        "2\t2\t",
        "old name.rs",
        "new name.rs",
    ]);
    let [renamed] = &files[..] else {
        panic!("one file")
    };
    assert_eq!(renamed.path.as_str(), "new name.rs");
    assert_eq!(renamed.old_path().as_str(), "old name.rs");
    assert_eq!(
        renamed.delta,
        Delta::Text {
            additions: 2,
            deletions: 2
        }
    );
}

#[test]
fn binary_files_have_dashes_and_gitlinks_are_flagged() {
    let files = parse(&[
        &format!(":100644 100644 {A} {B} M"),
        "logo.png",
        &format!(":160000 160000 {A} {B} M"),
        "vendor/lib",
        &format!(":160000 000000 {A} {ZEROS} D"),
        "removed-lib",
        "-\t-\tlogo.png",
        "1\t1\tvendor/lib",
        "1\t0\tremoved-lib",
    ]);
    assert!(files[0].is_binary() && !files[0].submodule);
    assert!(files[1].submodule);
    assert!(files[2].submodule && files[2].kind.is_deleted());
}

#[test]
fn a_path_with_a_tab_keeps_its_whole_name() {
    let files = parse(&[
        &format!(":100644 100644 {A} {B} M"),
        "tab\there.txt",
        "4\t0\ttab\there.txt",
    ]);
    assert_eq!(
        files[0].delta,
        Delta::Text {
            additions: 4,
            deletions: 0
        }
    );
}

#[test]
fn an_empty_listing_has_no_files_and_garbage_is_an_error_not_a_panic() {
    assert!(parse(&[]).is_empty());
    for bad in [
        &[":100644 100644"][..],
        &[":100644 100644 a b M"],
        &[&format!(":100644 100644 {A} {B} X"), "p"],
        &[&format!(":100644 100644 {A} {B} M")],
        &["no tabs here"],
        &[&format!(":100644 100644 {A} {B} M"), "p", "x\ty\tp"],
    ] {
        assert!(parse_raw_numstat(&records(bad)).is_err(), "{bad:?}");
    }
}
