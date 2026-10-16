use super::*;
use crate::domains::comments::sample_threads::{line, line_anchor, path, range};
use crate::domains::comments::storage::scratch_store::{
    all_threads, anchor, body, file_anchor, open, review_of,
};
use crate::domains::comments::threads::thread::Freshness;
use crate::kernel::side::Side;

#[tokio::test]
async fn relocates_flags_stale_and_unflags_when_text_returns() {
    let dir = tempfile::tempdir().unwrap();
    let store = open(&dir, "working").await;
    let id = store
        .add_thread(anchor(), body("hello"))
        .await
        .unwrap()
        .id()
        .clone();
    store.reply(&id, body("follows the anchor")).await.unwrap();
    let start = |thread: CommentThread| thread.anchor().start_line().unwrap().get();
    assert!(
        store
            .relocate_all(&review_of(&store, "z\na\nb\n", None).await)
            .await
            .unwrap()
    );
    assert_eq!(start(store.get(&id).await.unwrap().unwrap()), 3);
    assert!(
        store
            .relocate_all(&review_of(&store, "nothing here\n", None).await)
            .await
            .unwrap()
    );
    let stale = store.get(&id).await.unwrap().unwrap();
    assert_eq!(
        stale.freshness(),
        Freshness::Stale {
            from_line: Some(line(3))
        }
    );
    assert!(
        store
            .relocate_all(&review_of(&store, "b\n", None).await)
            .await
            .unwrap()
    );
    let back = store.get(&id).await.unwrap().unwrap();
    assert!(!back.is_stale());
    assert_eq!(start(back), 1);
    assert!(
        !store
            .relocate_all(&review_of(&store, "b\n", None).await)
            .await
            .unwrap()
    );
}

#[tokio::test]
async fn flags_a_thread_stale_when_its_text_survives_but_leaves_the_shown_ranges() {
    let dir = tempfile::tempdir().unwrap();
    let store = open(&dir, "working").await;
    let id = store
        .add_thread(anchor(), body("hello"))
        .await
        .unwrap()
        .id()
        .clone();
    let contents = "a\nb\nc\nd\ne\n";
    assert!(
        store
            .relocate_all(&review_of(&store, contents, Some(vec![range(4, 5)])).await)
            .await
            .unwrap()
    );
    let thread = store.get(&id).await.unwrap().unwrap();
    assert!(thread.is_stale());
    assert_eq!(thread.anchor().start_line(), Some(line(2)));
    assert!(
        store
            .relocate_all(&review_of(&store, contents, Some(vec![range(1, 3)])).await)
            .await
            .unwrap()
    );
    assert!(!store.get(&id).await.unwrap().unwrap().is_stale());
    assert!(
        store
            .relocate_all(&review_of(&store, contents, Some(Vec::new())).await)
            .await
            .unwrap()
    );
    assert!(store.get(&id).await.unwrap().unwrap().is_stale());
}

#[tokio::test]
async fn a_file_thread_is_stale_while_its_file_is_out_of_the_review_whatever_the_contents_say() {
    let dir = tempfile::tempdir().unwrap();
    let store = open(&dir, "working").await;
    let id = store
        .add_thread(file_anchor(), body("hello"))
        .await
        .unwrap()
        .id()
        .clone();
    let review = |present: bool| {
        let mut reviewed = ReviewedFiles::new();
        reviewed.record_file(path("a.py"), present);
        reviewed
    };
    assert!(!store.relocate_all(&review(true)).await.unwrap());
    let fresh = store.get(&id).await.unwrap().unwrap();
    assert_eq!((fresh.anchor(), fresh.is_stale()), (&file_anchor(), false));
    assert!(store.relocate_all(&review(false)).await.unwrap());
    let stale = store.get(&id).await.unwrap().unwrap();
    assert_eq!(stale.freshness(), Freshness::Stale { from_line: None });
    assert!(store.relocate_all(&review(true)).await.unwrap());
    assert!(!store.get(&id).await.unwrap().unwrap().is_stale());
}

#[tokio::test]
async fn removes_stale_threads_only() {
    let dir = tempfile::tempdir().unwrap();
    let store = open(&dir, "working").await;
    let missing = Anchor::Line(line_anchor("a.py", Side::New, 2, "missing"));
    let gone = store
        .add_thread(missing, body("hello"))
        .await
        .unwrap()
        .id()
        .clone();
    let kept = store
        .add_thread(anchor(), body("hello"))
        .await
        .unwrap()
        .id()
        .clone();
    store
        .relocate_all(&review_of(&store, "a\nb\n", None).await)
        .await
        .unwrap();
    assert!(store.get(&gone).await.unwrap().unwrap().is_stale());
    assert_eq!(store.remove_stale().await.unwrap(), 1);
    assert_eq!(store.remove_stale().await.unwrap(), 0);
    let ids: Vec<ThreadId> = all_threads(&store)
        .await
        .iter()
        .map(|t| t.id().clone())
        .collect();
    assert_eq!(ids, [kept]);
}

#[tokio::test]
async fn relocation_leaves_a_thread_alone_when_nobody_read_its_file() {
    let dir = tempfile::tempdir().unwrap();
    let store = open(&dir, "working").await;
    let id = store
        .add_thread(anchor(), body("hello"))
        .await
        .unwrap()
        .id()
        .clone();
    assert!(!store.relocate_all(&ReviewedFiles::new()).await.unwrap());
    assert!(!store.get(&id).await.unwrap().unwrap().is_stale());
}
