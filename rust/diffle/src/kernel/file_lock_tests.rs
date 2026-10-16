use std::sync::atomic::{AtomicUsize, Ordering};

use super::*;

#[tokio::test]
async fn work_under_the_lock_never_overlaps() {
    let dir = tempfile::tempdir().unwrap();
    let file = Arc::new(dir.path().join("state.json"));
    let inside = Arc::new(AtomicUsize::new(0));
    let mut tasks = Vec::new();
    for _ in 0..8 {
        let (file, inside) = (Arc::clone(&file), Arc::clone(&inside));
        tasks.push(tokio::spawn(async move {
            with_file_lock(&file, || async {
                assert_eq!(inside.fetch_add(1, Ordering::SeqCst), 0);
                tokio::time::sleep(Duration::from_millis(5)).await;
                inside.fetch_sub(1, Ordering::SeqCst);
            })
            .await
            .unwrap();
        }));
    }
    for task in tasks {
        task.await.unwrap();
    }
    assert!(!lock_path(&file).exists());
}

#[tokio::test]
async fn returns_the_work_result_and_releases_the_lock() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("state.json");
    let answer = with_file_lock(&file, || async { 42 }).await.unwrap();
    assert_eq!(answer, 42);
    assert!(!lock_path(&file).exists());
}

#[tokio::test]
async fn takes_over_a_lock_whose_holder_is_dead() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("state.json");
    let mut child = std::process::Command::new("git")
        .arg("--version")
        .spawn()
        .unwrap();
    let dead_pid = child.id();
    child.wait().unwrap();
    std::fs::write(lock_path(&file), format!("{dead_pid}\n")).unwrap();
    let answer = with_file_lock(&file, || async { "taken over" })
        .await
        .unwrap();
    assert_eq!(answer, "taken over");
}

#[tokio::test]
async fn waits_for_a_live_holder_then_times_out_naming_the_lock() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("state.json");
    std::fs::write(lock_path(&file), format!("{}\n", process::id())).unwrap();
    let error = acquire(&lock_path(&file), Duration::from_millis(50))
        .await
        .unwrap_err();
    assert!(matches!(&error, LockError::TimedOut(path) if *path == lock_path(&file)));
}
