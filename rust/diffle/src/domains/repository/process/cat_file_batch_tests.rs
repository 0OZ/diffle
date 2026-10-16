use super::*;
use crate::domains::repository::test_repo::TestRepo;

/// Larger than any pipe buffer or fair-scheduling budget, so a single poll cannot read it all.
const CANCELLED_BODY: usize = 16 * 1024 * 1024;

fn reader(fixture: &TestRepo) -> ObjectReader {
    ObjectReader::new(fixture.path())
}

#[tokio::test]
async fn many_reads_share_one_process() {
    let fixture = TestRepo::init();
    fixture.commit("a.txt", "alpha\n");
    fixture.commit("b.txt", "beta\n");
    let objects = reader(&fixture);
    for _ in 0..20 {
        let a = objects
            .read("HEAD:a.txt", usize::MAX)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(a.kind(), ObjectKind::Blob);
        assert_eq!(a.head(), b"alpha\n");
        let b = objects
            .read("HEAD:b.txt", usize::MAX)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(b.head(), b"beta\n");
    }
    assert_eq!(objects.spawns(), 1);
    objects.shutdown().await;
}

#[tokio::test]
async fn a_missing_name_is_absent_and_the_stream_stays_in_step() {
    let fixture = TestRepo::init();
    fixture.commit("a.txt", "alpha\n");
    let objects = reader(&fixture);
    assert_eq!(
        objects
            .read("HEAD:no such file.txt", usize::MAX)
            .await
            .unwrap(),
        None
    );
    assert_eq!(
        objects.read("HEAD:missing", usize::MAX).await.unwrap(),
        None
    );
    let a = objects
        .read("HEAD:a.txt", usize::MAX)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(a.head(), b"alpha\n");
    let tree = objects.read("HEAD:", usize::MAX).await.unwrap().unwrap();
    assert_eq!(tree.kind(), ObjectKind::Tree);
    objects.shutdown().await;
}

#[tokio::test]
async fn keeps_only_the_asked_prefix_but_reports_the_size() {
    let fixture = TestRepo::init();
    fixture.commit("big.txt", &"x".repeat(200_000));
    fixture.commit("small.txt", "tail\n");
    let objects = reader(&fixture);
    let big = objects.read("HEAD~1:big.txt", 10).await.unwrap().unwrap();
    assert_eq!(
        (big.size(), big.head()),
        (200_000, "x".repeat(10).as_bytes())
    );
    let small = objects
        .read("HEAD:small.txt", usize::MAX)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(small.head(), b"tail\n");
    objects.shutdown().await;
}

#[tokio::test]
async fn a_name_with_a_newline_is_answered_without_the_batch() {
    let fixture = TestRepo::init();
    fixture.commit("a.txt", "alpha\n");
    let objects = reader(&fixture);
    assert_eq!(
        objects.read("HEAD:a.txt\nb", usize::MAX).await.unwrap(),
        None
    );
    assert_eq!(objects.spawns(), 0);
}

#[tokio::test]
async fn the_process_is_reaped_when_idle_and_replaced_on_demand() {
    let fixture = TestRepo::init();
    fixture.commit("a.txt", "alpha\n");
    let objects = ObjectReader::reaping_after(fixture.path(), Duration::from_millis(40));
    assert!(
        objects
            .read("HEAD:a.txt", usize::MAX)
            .await
            .unwrap()
            .is_some()
    );
    sleep(Duration::from_millis(250)).await;
    assert!(objects.shared.slot.lock().await.batch.is_none());
    assert!(
        objects
            .read("HEAD:a.txt", usize::MAX)
            .await
            .unwrap()
            .is_some()
    );
    assert_eq!(objects.spawns(), 2);
    objects.shutdown().await;
}

/// A page that reloads drops the request it had in flight; the reply git was already writing
/// must not become the answer to the next, unrelated request.
#[tokio::test]
async fn a_read_dropped_in_flight_does_not_shift_the_next_answer() {
    let fixture = TestRepo::init();
    fixture.commit("big.txt", &"x".repeat(CANCELLED_BODY));
    fixture.commit("small.txt", "tail\n");
    let objects = reader(&fixture);
    objects.read("HEAD:small.txt", usize::MAX).await.unwrap();
    let mut abandoned = Box::pin(objects.read("HEAD~1:big.txt", 1));
    // One poll sends the name and starts on the reply, which is too large to finish at once.
    assert!(futures::poll!(abandoned.as_mut()).is_pending());
    drop(abandoned);
    let small = objects
        .read("HEAD:small.txt", usize::MAX)
        .await
        .unwrap()
        .unwrap();
    // The size first: a shifted answer is megabytes long and must not be printed whole.
    assert_eq!(small.size(), 5);
    assert_eq!(small.head(), b"tail\n");
    objects.shutdown().await;
}
