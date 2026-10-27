//! Isolated disk publication and language-server lifetimes over real Git repositories.
mod common;

use std::sync::{Arc, Mutex};

use common::{TestRepo, git_in, revspec, session};
use diffle::git::GitRepo;
use diffle::protocol::{EnterRequest, FollowRefs, RangeView, Side, Snapshot};

struct Owned {
    source: TestRepo,
    _clone: tempfile::TempDir,
    repo: Arc<GitRepo>,
    one: String,
    two: String,
    three: String,
}

impl Owned {
    async fn new() -> Self {
        let source = TestRepo::new();
        source.write("source.py", "import dependency\nbase = 0\n");
        source.write("dependency.py", "value = 'base'\n");
        source.commit("base");
        source.git(&["checkout", "-q", "-b", "feat"]);
        source.write("dependency.py", "value = 'one'\n");
        source.commit("dependency one");
        source.write("source.py", "import dependency\none = 1\n");
        source.commit("one");
        let one = source.git(&["rev-parse", "HEAD"]);
        source.write("dependency.py", "value = 'two'\n");
        source.commit("dependency two");
        source.write("source.py", "import dependency\ntwo = 2\n");
        source.commit("two");
        let two = source.git(&["rev-parse", "HEAD"]);
        source.git(&["checkout", "-q", "-b", "future"]);
        source.write("dependency.py", "value = 'three'\n");
        source.commit("three");
        let three = source.git(&["rev-parse", "HEAD"]);
        source.git(&["checkout", "-q", "main"]);
        let dir = tempfile::tempdir().unwrap();
        let repo = Arc::new(GitRepo::clone_into(&source.path.to_string_lossy(), dir.path()).await.unwrap());
        Self { source, _clone: dir, repo, one, two, three }
    }
}

fn observe(s: &Arc<diffle::session::Session>) -> Arc<Mutex<Vec<(String, String, String)>>> {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let record = seen.clone();
    let root = s.repo.root.clone();
    s.on_snapshot(move |snap| {
        record.lock().unwrap().push((
            snap.new_sha.clone(),
            git_in(&root, &["rev-parse", "HEAD"]),
            std::fs::read_to_string(root.join("dependency.py")).unwrap(),
        ));
    });
    seen
}

async fn begin(s: &Arc<diffle::session::Session>) -> Arc<Snapshot> {
    s.start(s.resolve(revspec(&["origin/main..origin/feat"])).await.unwrap()).await.unwrap()
}

#[tokio::test]
async fn owned_publication_checks_out_the_actual_shown_commit_before_listeners_run() {
    let t = Owned::new().await;
    let (s, _) = session(&t.repo, true, 3, FollowRefs::Off);
    let seen = observe(&s);
    let whole = begin(&s).await;
    let focused = s.show(RangeView::Commit { sha: t.one.clone() }).await.unwrap();
    let again = s.show(RangeView::Whole).await.unwrap();
    assert_eq!(
        *seen.lock().unwrap(),
        [
            (t.two.clone(), t.two.clone(), "value = 'two'\n".into()),
            (t.one.clone(), t.one.clone(), "value = 'one'\n".into()),
            (t.two.clone(), t.two.clone(), "value = 'two'\n".into()),
        ],
        "every listener must see the snapshot's commit on disk"
    );
    assert_eq!((&whole.head_sha, &focused.head_sha, &again.head_sha), (&t.two, &t.one, &t.two));
    assert!(whole.version < focused.version && focused.version < again.version);
    assert_eq!(t.source.git(&["branch", "--show-current"]), "main", "caller checkout stays untouched");
    s.close().await;
}

#[tokio::test]
async fn failed_checkout_that_keeps_head_does_not_disable_the_session() {
    let t = Owned::new().await;
    let (s, _) = session(&t.repo, false, 3, FollowRefs::Off);
    let before = begin(&s).await;
    let lock = t.repo.git_dir.join("index.lock");
    std::fs::write(&lock, "locked").unwrap();
    assert!(s.show(RangeView::Commit { sha: t.one.clone() }).await.is_err());
    std::fs::remove_file(lock).unwrap();
    assert!(s.workspace_read().await.is_ok(), "unchanged checkout must remain usable");
    assert_eq!(s.snapshotter().unwrap().current().await.unwrap().new_sha, before.new_sha);
    s.close().await;
}

#[tokio::test]
async fn owned_head_range_survives_focus_context_and_return_to_whole() {
    let t = Owned::new().await;
    git_in(&t.repo.root, &["checkout", "-q", "-b", "review", &t.two]);
    let (s, _) = session(&t.repo, false, 3, FollowRefs::Off);
    let first = s.start(s.resolve(revspec(&["origin/main..HEAD"])).await.unwrap()).await.unwrap();
    let focused = s.show(RangeView::Commit { sha: t.one.clone() }).await.unwrap();
    assert_eq!(focused.commits.new_sha, first.commits.new_sha);
    s.set_context(9).await.unwrap();
    let context = s.snapshotter().unwrap().current().await.unwrap();
    assert_eq!(context.commits.new_sha, t.two, "context must resolve the logical HEAD, not focused disk HEAD");
    assert_eq!(context.commits.list.len(), first.commits.list.len());
    let whole = s.show(RangeView::Whole).await.unwrap();
    assert_eq!((whole.new_sha.as_str(), whole.head_sha.as_str()), (&*t.two, &*t.two));
    assert_eq!(whole.comment_key, first.comment_key, "range comment identity must survive physical checkout");
    assert_eq!(
        serde_json::to_value(&whole.review).unwrap(),
        serde_json::to_value(&first.review).unwrap(),
        "keep the logical user-facing review"
    );
    s.close().await;
}

#[tokio::test]
async fn owned_head_relative_endpoints_stay_fixed_but_named_refs_remain_live() {
    let t = Owned::new().await;
    t.repo.checkout_owned(&t.two).await.unwrap();
    let (s, _) = session(&t.repo, false, 3, FollowRefs::Off);
    let first = s.start(s.resolve(revspec(&["HEAD~4..HEAD"])).await.unwrap()).await.unwrap();
    s.show(RangeView::Commit { sha: t.one.clone() }).await.unwrap();
    let whole = s.show(RangeView::Whole).await.unwrap();
    assert_eq!(
        (&whole.old_sha, &whole.new_sha, &whole.comment_key),
        (&first.old_sha, &first.new_sha, &first.comment_key)
    );
    s.enter(revspec(&["origin/main..origin/feat"])).await.unwrap();
    git_in(&t.repo.root, &["update-ref", "refs/remotes/origin/feat", &t.three]);
    assert_eq!(s.reload().await.unwrap().new_sha, t.three, "named branch refs must not be pinned with HEAD");
    s.close().await;
}

#[tokio::test]
async fn owned_refresh_and_context_publish_a_coherent_checkout() {
    let t = Owned::new().await;
    let (s, _) = session(&t.repo, false, 3, FollowRefs::Off);
    begin(&s).await;
    let seen = observe(&s);
    git_in(&t.repo.root, &["update-ref", "refs/remotes/origin/feat", &t.three]);
    s.refresh().await;
    let refreshed = s.snapshotter().unwrap().current().await.unwrap();
    assert_eq!(refreshed.head_sha, t.three, "refresh must synchronize disk, not merely compute Git objects");
    s.set_context(7).await.unwrap();
    let context = s.snapshotter().unwrap().current().await.unwrap();
    assert_eq!((context.context, context.new_sha.as_str(), context.head_sha.as_str()), (7, &*t.three, &*t.three));
    assert_eq!(*seen.lock().unwrap(), vec![(t.three.clone(), t.three.clone(), "value = 'three'\n".into()); 2]);
    s.close().await;
}

#[tokio::test]
async fn owned_rejected_view_and_failed_activation_keep_previous_review_and_disk() {
    let t = Owned::new().await;
    let (s, _) = session(&t.repo, true, 3, FollowRefs::Off);
    let first = begin(&s).await;
    assert!(s.show(RangeView::Uncommitted).await.is_err());
    assert!(
        s.enter(revspec(&["worktree..origin/main"])).await.is_err(),
        "cannot mutate the worktree used as the old side"
    );
    let file = s.comments().unwrap().file().to_owned();
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    std::fs::write(&file, "broken JSON").unwrap();
    assert!(s.show(RangeView::Commit { sha: t.one.clone() }).await.is_err(), "comment relocation must fail");
    assert_eq!(git_in(&t.repo.root, &["rev-parse", "HEAD"]), t.two, "failed activation must roll disk back");
    assert_eq!(std::fs::read_to_string(t.repo.root.join("dependency.py")).unwrap(), "value = 'two'\n");
    assert!(Arc::ptr_eq(&first, &s.snapshotter().unwrap().current().await.unwrap()), "old snapshot stays active");
    std::fs::remove_file(file).unwrap();
    s.close().await;
}

#[tokio::test]
async fn ordinary_dirty_worktree_is_not_synchronized_and_working_reads_still_use_the_allowlist() {
    let t = Owned::new().await;
    t.source.write("source.py", "dirty tracked\n");
    t.source.write("private.txt", "untracked\n");
    let before = t.source.git(&["status", "--porcelain=v1"]);
    let head = t.source.git(&["rev-parse", "HEAD"]);
    let repo = Arc::new(t.source.open().await);
    let (s, _) = session(&repo, false, 3, FollowRefs::Off);
    let snap = s.start(s.resolve(revspec(&["main..feat"])).await.unwrap()).await.unwrap();
    assert_eq!(snap.head_sha, head);
    assert_ne!(snap.new_sha, snap.head_sha, "ordinary wrong-checkout blocker must remain meaningful");
    s.refresh().await;
    s.set_context(5).await.unwrap();
    assert_eq!(t.source.git(&["status", "--porcelain=v1"]), before);
    assert_eq!(std::fs::read_to_string(t.source.path.join("source.py")).unwrap(), "dirty tracked\n");
    let working = s.enter(EnterRequest::Working).await.unwrap();
    assert_eq!(s.read_side(&working, "source.py", Side::New).await.unwrap(), Some(b"dirty tracked\n".to_vec()));
    assert!(s.read_side(&working, ".git/config", Side::New).await.unwrap().is_none());
    s.close().await;
}

#[tokio::test]
async fn owned_snapshot_freezes_endpoints_before_the_checkout_lifecycle_hook() {
    use futures::FutureExt;
    let t = Owned::new().await;
    let (s, _) = session(&t.repo, false, 3, FollowRefs::Off);
    let root = t.repo.root.clone();
    let three = t.three.clone();
    s.before_checkout(move || {
        let (root, three) = (root.clone(), three.clone());
        async move {
            git_in(&root, &["update-ref", "refs/remotes/origin/feat", &three]);
        }
        .boxed()
    });
    let snap = begin(&s).await;
    assert_eq!(
        (&snap.new_sha, &snap.head_sha, &snap.commits.new_sha),
        (&t.two, &t.two, &t.two),
        "checkout must not resolve the view a second time"
    );
    assert_eq!(git_in(&t.repo.root, &["rev-parse", "origin/feat"]), t.three, "the ref really moved during preparation");
    s.close().await;
}

#[tokio::test]
async fn owned_working_mode_watches_edits_but_will_not_discard_them_on_revision_change() {
    let t = Owned::new().await;
    let (s, _) = session(&t.repo, true, 3, FollowRefs::Off);
    begin(&s).await;
    s.enter(EnterRequest::Working).await.unwrap();
    let previous = s.snapshotter().unwrap().current().await.unwrap().version;
    std::fs::write(t.repo.root.join("source.py"), "my uncommitted work\n").unwrap();
    common::eventually("owned working edit published", async || {
        s.snapshotter().unwrap().current().await.unwrap().version > previous
    })
    .await;
    s.set_context(8).await.unwrap();
    let kept = s.snapshotter().unwrap().current().await.unwrap();
    assert_eq!(kept.new_sha, diffle::git::WORKTREE);
    assert!(
        s.enter(revspec(&[&format!("{}^!", t.one)])).await.is_err(),
        "dirty working mode must not lose edits to an owned checkout"
    );
    assert_eq!(std::fs::read_to_string(t.repo.root.join("source.py")).unwrap(), "my uncommitted work\n");
    assert_eq!(git_in(&t.repo.root, &["rev-parse", "HEAD"]), t.two);
    assert!(Arc::ptr_eq(&kept, &s.snapshotter().unwrap().current().await.unwrap()));
    s.close().await;
}

#[tokio::test]
async fn owned_edited_committed_checkout_rejects_same_revision_publication_without_discarding_edits() {
    let t = Owned::new().await;
    let (s, _) = session(&t.repo, false, 3, FollowRefs::Off);
    let first = begin(&s).await;
    std::fs::write(t.repo.root.join("source.py"), "edited outside working mode\n").unwrap();
    assert!(
        s.enter(revspec(&[&format!("{}^!", t.one)])).await.is_err(),
        "changing revisions cannot discard edits made outside working mode"
    );
    assert!(s.set_context(7).await.is_err(), "same-revision update cannot silently publish contradictory disk");
    assert_eq!(s.context(), 3, "failed context update must not commit its setting");
    assert!(Arc::ptr_eq(&first, &s.snapshotter().unwrap().current().await.unwrap()));
    assert_eq!(std::fs::read_to_string(t.repo.root.join("source.py")).unwrap(), "edited outside working mode\n");
    s.close().await;
}

#[tokio::test]
async fn failed_owned_rollback_fails_closed_instead_of_exposing_an_incoherent_snapshot() {
    use futures::FutureExt;
    use std::sync::atomic::{AtomicUsize, Ordering};
    let t = Owned::new().await;
    let (s, _) = session(&t.repo, false, 3, FollowRefs::Off);
    begin(&s).await;
    let file = s.comments().unwrap().file().to_owned();
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    std::fs::write(&file, "broken JSON").unwrap();
    let lock = t.repo.git_dir.join("index.lock");
    s.before_checkout({
        let calls = AtomicUsize::new(0);
        let lock = lock.clone();
        move || {
            let rollback = calls.fetch_add(1, Ordering::SeqCst) == 1;
            let lock = lock.clone();
            async move {
                if rollback {
                    std::fs::write(lock, "other git operation").unwrap();
                }
            }
            .boxed()
        }
    });
    let failed = s.show(RangeView::Commit { sha: t.one.clone() }).await.unwrap_err();
    assert!(failed.to_string().contains("restoring isolated checkout also failed"), "{failed}");
    assert!(s.workspace_read().await.is_err(), "queries must fail closed after rollback failed");
    assert!(s.snapshotter().is_err(), "the previous snapshot must not stay exposed against the wrong disk");
    std::fs::remove_file(lock).unwrap();
    s.close().await;
}

#[cfg(unix)]
mod language_servers {
    use super::*;
    use common::lsp::{eventually, fake_env, fake_servers, flush};
    use diffle::lsp::pool::{LspPool, PoolOptions};
    use diffle::protocol::{LanguageId, LspPosition, LspState};
    use diffle::server::{ApiDeps, Server, ServerOptions, hub::Hub};
    use diffle::user_config::UserConfigStore;
    use futures::FutureExt;
    use serde_json::json;

    fn pool(s: &Arc<diffle::session::Session>) -> Arc<LspPool> {
        let pool = LspPool::deferred(options(s));
        pool.attach(s);
        pool
    }

    fn options(s: &Arc<diffle::session::Session>) -> PoolOptions {
        let (reader, checker) = (Arc::downgrade(s), Arc::downgrade(s));
        PoolOptions {
            root: s.repo.root.clone(),
            read: Arc::new(move |path| {
                let s = reader.clone();
                async move {
                    let s = s.upgrade()?;
                    let snap = s.snapshotter().ok()?.current().await.ok()?;
                    let bytes = s.read_side(&snap, &path, Side::New).await.ok()??;
                    String::from_utf8(bytes).ok()
                }
                .boxed()
            }),
            has: Arc::new(move |path| {
                let s = checker.clone();
                async move {
                    let Some(s) = s.upgrade() else { return false };
                    let Ok(snapshotter) = s.snapshotter() else { return false };
                    let Ok(snap) = snapshotter.current().await else { return false };
                    s.has_side(&snap, &path, Side::New)
                }
                .boxed()
            }),
            on_status: Arc::new(|_| {}),
            overrides: [(LanguageId::Python, "fake".into())].into(),
            preload: vec![LanguageId::Python],
            lookup: Arc::new(|_| None),
            max_open: None,
        }
    }

    async fn ready(pool: &Arc<LspPool>) -> Arc<diffle::lsp::pool_server::PoolServer> {
        eventually("owned server ready", || pool.status().servers.iter().any(|s| s.state == LspState::Ready)).await;
        let server = pool.server("fake").unwrap();
        flush(server.bridge.process()).await;
        server
    }

    fn at() -> LspPosition {
        serde_json::from_value(json!({ "path": "source.py", "line": 2, "col": 0 })).unwrap()
    }

    #[tokio::test]
    async fn close_settles_even_when_the_language_server_cannot_be_spawned() {
        use diffle::lsp::process::{LspProcess, ProcessOptions};
        let t = Owned::new().await;
        let process = LspProcess::start(ProcessOptions {
            command: t.repo.root.join("missing-language-server").to_string_lossy().into_owned(),
            root: t.repo.root.clone(),
            settings: None,
            initialization_options: None,
            on_status: Arc::new(|_| {}),
        });
        let failure = process.connection().await.err().expect("missing executable must fail startup");
        assert!(failure.to_string().contains("cannot start"), "{failure}");
        tokio::time::timeout(std::time::Duration::from_millis(1500), process.close())
            .await
            .expect("failed spawn must settle the startup/exit barrier, not hang close");
    }

    #[tokio::test]
    async fn closing_before_background_launch_settles_cannot_leave_a_later_child() {
        fake_servers().expect("node is required for owned LSP integration coverage");
        let t = Owned::new().await;
        let (s, _) = session(&t.repo, false, 3, FollowRefs::Off);
        let pool = LspPool::new(options(&s));
        let process = pool.server("fake").unwrap().bridge.process().clone();
        pool.close().await;
        let connection = process.connection().await;
        pool.close().await;
        s.close().await;
        assert!(connection.is_err(), "a close before launch must settle startup as unavailable, not spawn later");
    }

    #[tokio::test]
    async fn close_interrupts_tracking_and_reaps_a_noninitializing_node_server() {
        fake_servers().expect("node is required for owned LSP integration coverage");
        let t = Owned::new().await;
        std::fs::write(t.repo.git_dir.join("info/exclude"), ".fake-env*\n.query-*\n").unwrap();
        let started = t.repo.root.join(".query-started");
        let release = t.repo.root.join(".query-release");
        fake_env(
            &t.repo.root,
            None,
            &[
                ("FAKE_LSP_QUERY_METHOD", "initialize"),
                ("FAKE_LSP_QUERY_STARTED", &started.to_string_lossy()),
                ("FAKE_LSP_QUERY_RELEASE", &release.to_string_lossy()),
            ],
        );
        let (s, _) = session(&t.repo, false, 3, FollowRefs::Off);
        let pool = pool(&s);
        let snap = begin(&s).await;
        eventually("real Node server latched in initialize", || started.exists()).await;
        let pid: i32 = std::fs::read_to_string(&started).unwrap().parse().unwrap();
        let tracking = tokio::spawn({
            let pool = pool.clone();
            async move { pool.track(&snap).await }
        });
        let mut closing = tokio::spawn({
            let pool = pool.clone();
            async move { pool.close().await }
        });
        let completed = tokio::time::timeout(std::time::Duration::from_millis(1500), &mut closing).await;
        let in_budget = completed.is_ok();
        // Probe before releasing initialize or yielding again: cleanup cannot hide an early close return.
        // SAFETY: signal 0 only probes our known PID; WNOHANG cannot reap a live process.
        let alive = unsafe { libc::kill(pid, 0) };
        let existence_error = std::io::Error::last_os_error().raw_os_error();
        let mut status = 0;
        // SAFETY: a nonblocking probe of our known child verifies close already reaped it.
        let waited = unsafe { libc::waitpid(pid, &mut status, libc::WNOHANG) };
        let wait_error = std::io::Error::last_os_error().raw_os_error();
        // Release only to clean up a broken implementation, before reporting its failed deadline.
        std::fs::write(release, "release").unwrap();
        if let Ok(result) = completed {
            result.unwrap();
        } else {
            closing.await.unwrap();
        }
        tracking.await.unwrap();
        s.close().await;
        assert!(in_budget, "close waited behind tracking's initialization lease instead of interrupting it");
        assert_eq!(
            (alive, existence_error),
            (-1, Some(libc::ESRCH)),
            "close returned with a live or zombie Node child"
        );
        assert_eq!((waited, wait_error), (-1, Some(libc::ECHILD)), "close must reap its child before returning");
    }

    #[tokio::test]
    async fn owned_revision_change_restarts_unopened_import_index_and_revokes_external_grants() {
        fake_servers().expect("node is required for owned LSP integration coverage");
        let t = Owned::new().await;
        std::fs::write(t.repo.git_dir.join("info/exclude"), ".fake-env*\n").unwrap();
        let outside = tempfile::tempdir().unwrap();
        let external = outside.path().join("external.py");
        std::fs::write(&external, "external = 1\n").unwrap();
        fake_env(
            &t.repo.root,
            None,
            &[("FAKE_LSP_INDEX_IMPORT", "dependency.py"), ("FAKE_LSP_EXTERNAL", &external.to_string_lossy())],
        );
        let (s, _) = session(&t.repo, false, 3, FollowRefs::Off);
        let pool = pool(&s);
        assert!(pool.status().servers.is_empty(), "isolated preload must not read the clone's initial checkout");
        s.start(s.resolve(revspec(&[&format!("{}^!", t.one)])).await.unwrap()).await.unwrap();
        let old = ready(&pool).await;
        let first = pool.hover(&at(), false).await.unwrap();
        assert!(first.contents.as_deref().unwrap().contains("value = 'one'"));
        assert!(
            !old.bridge.entries().iter().any(|e| e.message.starts_with("open dependency.py")),
            "import stays unopened"
        );
        assert!(
            old.bridge
                .entries()
                .iter()
                .any(|e| e.message == format!("indexed {}: value = 'one'", t.repo.root.display())),
            "server cwd is the isolated root"
        );
        pool.definition(&at()).await.unwrap();
        assert!(pool.named(&external.to_string_lossy()), "definition grants external access");
        s.enter(revspec(&[&format!("{}^!", t.two)])).await.unwrap();
        let new = ready(&pool).await;
        assert!(!Arc::ptr_eq(&old, &new), "a new revision must replace the entire server, not just its documents");
        assert!(pool.hover(&at(), false).await.unwrap().contents.as_deref().unwrap().contains("value = 'two'"));
        assert!(!pool.named(&external.to_string_lossy()), "old external grants must not survive the reset");
        s.set_context(9).await.unwrap();
        s.refresh().await;
        let same = ready(&pool).await;
        assert!(Arc::ptr_eq(&new, &same), "same-revision context and refresh must retain the warm process");
        let before_failure = s.snapshotter().unwrap().current().await.unwrap();
        let published = Arc::new(Mutex::new(Vec::new()));
        let record = published.clone();
        s.on_snapshot(move |snap| record.lock().unwrap().push(snap.version));
        let file = s.comments().unwrap().file().to_owned();
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(&file, "broken JSON").unwrap();
        assert!(s.enter(revspec(&[&format!("{}^!", t.one)])).await.is_err());
        let restored = ready(&pool).await;
        assert!(!Arc::ptr_eq(&same, &restored), "rollback must rebuild the old revision's index");
        assert!(pool.hover(&at(), false).await.unwrap().contents.as_deref().unwrap().contains("value = 'two'"));
        assert!(Arc::ptr_eq(&before_failure, &s.snapshotter().unwrap().current().await.unwrap()));
        assert!(published.lock().unwrap().is_empty(), "rollback must not publish a duplicate snapshot version");
        std::fs::remove_file(file).unwrap();
        pool.close().await;
        pool.track(&s.snapshotter().unwrap().current().await.unwrap()).await;
        assert!(
            Arc::ptr_eq(&restored, &pool.server("fake").unwrap()),
            "tracking after shutdown cannot spawn a replacement"
        );
        s.close().await;
    }

    #[tokio::test]
    async fn queued_old_tracking_rechecks_identity_after_the_checkout_gate() {
        use std::sync::atomic::{AtomicBool, Ordering};
        fake_servers().expect("node is required for owned LSP integration coverage");
        let t = Owned::new().await;
        std::fs::write(t.repo.git_dir.join("info/exclude"), ".fake-env*\n").unwrap();
        fake_env(&t.repo.root, None, &[("FAKE_LSP_INDEX_IMPORT", "dependency.py")]);
        let (s, _) = session(&t.repo, false, 3, FollowRefs::Off);
        let pool = pool(&s);
        let old = begin(&s).await;
        ready(&pool).await;
        let entered = Arc::new(tokio::sync::Notify::new());
        let release = Arc::new(tokio::sync::Notify::new());
        s.before_checkout({
            let (entered, release, pool) = (entered.clone(), release.clone(), pool.clone());
            move || {
                let (entered, release, pool) = (entered.clone(), release.clone(), pool.clone());
                async move {
                    pool.reset().await;
                    entered.notify_one();
                    release.notified().await;
                }
                .boxed()
            }
        });
        let showing = tokio::spawn({
            let (s, one) = (s.clone(), t.one.clone());
            async move { s.show(RangeView::Commit { sha: one }).await }
        });
        entered.notified().await;
        let ran = Arc::new(AtomicBool::new(false));
        let tracking = s.track_snapshot(old, {
            let (ran, pool) = (ran.clone(), pool.clone());
            move |snap| {
                async move {
                    ran.store(true, Ordering::SeqCst);
                    pool.track(&snap).await;
                }
                .boxed()
            }
        });
        release.notify_one();
        showing.await.unwrap().unwrap();
        tracking.await.unwrap();
        assert!(!ran.load(Ordering::SeqCst), "a queued job for the old snapshot must not track after checkout");
        ready(&pool).await;
        assert!(pool.hover(&at(), false).await.unwrap().contents.as_deref().unwrap().contains("value = 'one'"));
        pool.close().await;
        s.close().await;
    }

    #[tokio::test]
    async fn http_query_finishes_before_owned_checkout_and_next_query_uses_new_index() {
        let answer = query_crossing_transition(reqwest::Method::POST, "/api/lsp/hover", "textDocument/hover").await;
        assert!(answer["contents"].as_str().unwrap().contains("value = 'two'"));
    }

    #[tokio::test]
    async fn http_graph_holds_the_checkout_lease_across_its_language_server_queries() {
        let answer =
            query_crossing_transition(reqwest::Method::GET, "/api/lsp/graph", "textDocument/documentSymbol").await;
        assert_eq!(answer["nodes"], json!([]), "the graph finishes its old-snapshot outline pass");
        assert!(answer["inconclusive"].is_null(), "the old server was not killed in the middle of the graph");
    }

    async fn query_crossing_transition(method: reqwest::Method, path: &str, held: &str) -> serde_json::Value {
        fake_servers().expect("node is required for owned LSP integration coverage");
        let t = Owned::new().await;
        std::fs::write(t.repo.git_dir.join("info/exclude"), ".fake-env*\n.query-*\n").unwrap();
        let started = t.repo.root.join(".query-started");
        let release = t.repo.root.join(".query-release");
        fake_env(
            &t.repo.root,
            None,
            &[
                ("FAKE_LSP_INDEX_IMPORT", "dependency.py"),
                ("FAKE_LSP_QUERY_STARTED", &started.to_string_lossy()),
                ("FAKE_LSP_QUERY_RELEASE", &release.to_string_lossy()),
                ("FAKE_LSP_QUERY_METHOD", held),
                ("FAKE_LSP_GRAPH", "1"),
            ],
        );
        let (s, _) = session(&t.repo, false, 3, FollowRefs::Off);
        let pool = pool(&s);
        let hub = Hub::new();
        let config = Arc::new(UserConfigStore::open(t.repo.git_dir.join("user-config.json")).await);
        let deps = ApiDeps { session: s.clone(), config, extra_auto_viewed: Vec::new(), hub, lsp: Some(pool.clone()) };
        let opts = ServerOptions { port: 0, probe: false, host: "127.0.0.1".into(), allowed_origin: None };
        let (server, base) = Server::listen(deps, opts).await.unwrap();
        begin(&s).await;
        ready(&pool).await;
        let url = format!("{}{path}", base.trim_end_matches('/'));
        let querying =
            tokio::spawn(async move { reqwest::Client::new().request(method, url).json(&at()).send().await.unwrap() });
        eventually("query inside old server", || started.exists()).await;
        let mut showing = tokio::spawn({
            let s = s.clone();
            let one = t.one.clone();
            async move { s.show(RangeView::Commit { sha: one }).await }
        });
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(150), &mut showing).await.is_err(),
            "transition must await the query's read lease"
        );
        assert_eq!(git_in(&t.repo.root, &["rev-parse", "HEAD"]), t.two, "disk cannot change under an in-flight query");
        std::fs::write(&release, "go").unwrap();
        let answer = querying.await.unwrap();
        assert_eq!(answer.status(), 200);
        let answer = answer.json::<serde_json::Value>().await.unwrap();
        showing.await.unwrap().unwrap();
        ready(&pool).await;
        let next = reqwest::Client::new()
            .post(format!("{}/api/lsp/hover", base.trim_end_matches('/')))
            .json(&at())
            .send()
            .await
            .unwrap();
        assert_eq!(next.status(), 200);
        assert!(
            next.json::<serde_json::Value>().await.unwrap()["contents"].as_str().unwrap().contains("value = 'one'")
        );
        server.close().await;
        pool.close().await;
        s.close().await;
        answer
    }
}
