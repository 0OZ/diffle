//! Resolution against real repositories, shaped like the diagram in `resolve_endpoints`.

use super::resolve_endpoints::resolve_endpoints;
use super::resolve_review::resolve_review;
use crate::domains::repository::git_repo::GitRepo;
use crate::domains::repository::names::object_id::ObjectId;
use crate::domains::repository::names::rev_name::RevName;
use crate::domains::repository::names::tree_source::TreeSource;
use crate::domains::repository::test_repo::TestRepo;
use crate::domains::revisions::comparison::Liveness;
use crate::domains::revisions::entry_request::EntryRequest;
use crate::domains::revisions::revspec::Revspec;
use crate::domains::revisions::revspec_error::{ResolveError, RevspecError};

/// `A--B--C` on main, `B--D--E` on feat, with `HEAD` on feat.
struct Diamond {
    fixture: TestRepo,
    b: ObjectId,
    c: ObjectId,
    d: ObjectId,
    e: ObjectId,
}

impl Diamond {
    fn new() -> Self {
        let fixture = TestRepo::init();
        fixture.commit("a.txt", "a");
        let b = fixture.commit("b.txt", "b");
        fixture.git(&["switch", "--quiet", "--create", "feat"]);
        let d = fixture.commit("d.txt", "d");
        let e = fixture.commit("e.txt", "e");
        fixture.git(&["switch", "--quiet", "main"]);
        let c = fixture.commit("c.txt", "c");
        fixture.git(&["switch", "--quiet", "feat"]);
        Self {
            fixture,
            b,
            c,
            d,
            e,
        }
    }

    async fn repo(&self) -> GitRepo {
        GitRepo::open(self.fixture.path()).await.unwrap()
    }
}

fn spec(args: &[&str]) -> Revspec {
    let owned: Vec<String> = args.iter().map(|arg| (*arg).to_owned()).collect();
    Revspec::parse(&owned).unwrap()
}

fn object(id: &ObjectId) -> TreeSource {
    TreeSource::Object(id.clone())
}

async fn resolved(repo: &GitRepo, args: &[&str]) -> (TreeSource, TreeSource, Liveness) {
    let r = resolve_endpoints(repo, &spec(args)).await.unwrap();
    (r.old, r.new, r.live)
}

async fn key(repo: &GitRepo, args: &[&str]) -> String {
    let request = EntryRequest::Revspec(spec(args));
    let comparison = resolve_review(repo, &request).await.unwrap();
    comparison.comment_key().to_string()
}

#[tokio::test]
async fn resolves_every_row_of_the_diagram() {
    let g = Diamond::new();
    let repo = g.repo().await;
    let e = || object(&g.e);

    assert_eq!(
        resolved(&repo, &["main..HEAD"]).await,
        (object(&g.c), e(), Liveness::Refs)
    );
    assert_eq!(
        resolved(&repo, &["main...HEAD"]).await,
        (object(&g.b), e(), Liveness::Refs)
    );
    assert_eq!(
        resolved(&repo, &["main"]).await,
        (object(&g.b), e(), Liveness::Refs)
    );
    assert_eq!(
        resolved(&repo, &["HEAD..worktree"]).await,
        (e(), TreeSource::Worktree, Liveness::Worktree)
    );
    assert_eq!(
        resolved(&repo, &["worktree..HEAD"]).await,
        (TreeSource::Worktree, e(), Liveness::Worktree)
    );
    assert_eq!(
        resolved(&repo, &["main...worktree"]).await,
        (object(&g.b), TreeSource::Worktree, Liveness::Worktree)
    );
    assert_eq!(
        resolved(&repo, &[g.c.as_str(), g.e.as_str()]).await,
        (object(&g.c), e(), Liveness::Pinned)
    );
}

#[tokio::test]
async fn a_commit_is_shown_against_its_first_parent() {
    let g = Diamond::new();
    let repo = g.repo().await;
    assert_eq!(
        resolved(&repo, &[&format!("{}^!", g.e)]).await,
        (object(&g.d), object(&g.e), Liveness::Pinned)
    );
    assert_eq!(
        resolved(&repo, &["HEAD^!"]).await,
        (object(&g.d), object(&g.e), Liveness::Refs)
    );
}

#[tokio::test]
async fn the_side_before_a_root_commit_is_the_empty_tree() {
    let fixture = TestRepo::init();
    let root = fixture.commit("a.txt", "a");
    let repo = GitRepo::open(fixture.path()).await.unwrap();
    let (old, new, _) = resolved(&repo, &["HEAD^!"]).await;
    assert_eq!(old, TreeSource::Object(repo.empty_tree().await.unwrap()));
    assert_eq!(new, TreeSource::Object(root));
}

#[tokio::test]
async fn a_repository_without_commits_compares_the_worktree_with_the_empty_tree() {
    let fixture = TestRepo::init();
    let repo = GitRepo::open(fixture.path()).await.unwrap();
    assert_eq!(
        resolved(&repo, &["HEAD..worktree"]).await,
        (
            TreeSource::Object(repo.empty_tree().await.unwrap()),
            TreeSource::Worktree,
            Liveness::Worktree
        )
    );
    let head = RevName::head();
    let error = resolve_endpoints(&repo, &spec(&["HEAD...worktree"]))
        .await
        .unwrap_err();
    assert!(
        matches!(error, ResolveError::Revspec(RevspecError::UnknownRevision(rev)) if rev == head)
    );
}

#[tokio::test]
async fn names_what_could_not_be_resolved() {
    let g = Diamond::new();
    let repo = g.repo().await;
    let unknown = resolve_endpoints(&repo, &spec(&["nope..HEAD"]))
        .await
        .unwrap_err();
    assert_eq!(unknown.to_string(), "unknown revision: nope");

    g.fixture.git(&["switch", "--quiet", "--orphan", "other"]);
    g.fixture.commit("z.txt", "z");
    let unrelated = resolve_endpoints(&repo, &spec(&["main...other"]))
        .await
        .unwrap_err();
    assert_eq!(
        unrelated.to_string(),
        "no merge base between main and other"
    );
}

#[tokio::test]
async fn the_working_review_is_live_and_keyed_working() {
    let g = Diamond::new();
    let repo = g.repo().await;
    let comparison = resolve_review(&repo, &EntryRequest::Working).await.unwrap();
    assert_eq!(comparison.spec().to_string(), "HEAD..worktree");
    assert_eq!(comparison.live(), Liveness::Worktree);
    assert_eq!(comparison.comment_key().as_str(), "working");
}

#[tokio::test]
async fn a_shown_commit_is_pinned_by_its_hash() {
    let g = Diamond::new();
    let repo = g.repo().await;
    let request = EntryRequest::Revspec(spec(&["HEAD^!"]));
    let comparison = resolve_review(&repo, &request).await.unwrap();
    assert_eq!(comparison.spec().to_string(), format!("{}^!", g.e));
    assert_eq!(comparison.live(), Liveness::Pinned);
    assert_eq!(comparison.comment_key().as_str(), format!("commit:{}", g.e));
}

#[tokio::test]
async fn branches_key_by_full_ref_and_everything_else_by_what_it_resolved_to() {
    let g = Diamond::new();
    let repo = g.repo().await;
    assert_eq!(
        key(&repo, &["main...feat"]).await,
        "range:refs/heads/main...refs/heads/feat"
    );
    assert_eq!(
        key(&repo, &["main"]).await,
        "range:refs/heads/main...refs/heads/feat"
    );
    assert_eq!(
        key(&repo, &["main..worktree"]).await,
        "range:refs/heads/main..worktree"
    );
    assert_eq!(
        key(&repo, &[g.c.as_str(), g.e.as_str()]).await,
        format!("range:{}..{}", g.c, g.e)
    );
    g.fixture.git(&["switch", "--quiet", "--detach"]);
    assert_eq!(
        key(&repo, &["main...HEAD"]).await,
        format!("range:refs/heads/main...{}", g.e)
    );
}

#[tokio::test]
async fn branches_tracking_github_key_by_repository_so_forks_stay_apart() {
    let g = Diamond::new();
    g.fixture
        .git(&["remote", "add", "origin", "git@github.com:Owner/Repo.git"]);
    for branch in ["main", "feat"] {
        g.fixture
            .git(&["config", &format!("branch.{branch}.remote"), "origin"]);
        g.fixture.git(&[
            "config",
            &format!("branch.{branch}.merge"),
            &format!("refs/heads/{branch}"),
        ]);
    }
    let repo = g.repo().await;
    assert_eq!(
        key(&repo, &["main...feat"]).await,
        "range:owner/repo:main...owner/repo:feat"
    );
    assert_eq!(
        key(&repo, &["main..worktree"]).await,
        "range:owner/repo:main..worktree"
    );
}
