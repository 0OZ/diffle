use super::*;
use crate::domains::changes::changed_files::changed_files;
use crate::domains::repository::names::tree_source::TreeSource;
use crate::domains::repository::test_repo::TestRepo;

struct Scene {
    /// Keeps the temporary repository alive.
    _fixture: TestRepo,
    repo: GitRepo,
    sides: DiffSides,
    files: Vec<ChangedFile>,
}

async fn scene() -> Scene {
    let fixture = TestRepo::init();
    let base = fixture.commit("tracked.txt", "one\n");
    fixture.commit("other.txt", "keep\n");
    std::fs::write(fixture.path().join("tracked.txt"), "one\ntwo\n").unwrap();
    std::fs::write(fixture.path().join("fresh.txt"), "brand new\n").unwrap();
    std::fs::write(fixture.path().join("staged.txt"), "staged\n").unwrap();
    fixture.git(&["add", "staged.txt"]);
    let repo = GitRepo::open(fixture.path()).await.unwrap();
    let old = TreeSource::Object(base);
    let files = changed_files(&repo, &old, &TreeSource::Worktree)
        .await
        .unwrap();
    let sides = DiffSides::new(&old, &TreeSource::Worktree);
    Scene {
        _fixture: fixture,
        repo,
        sides,
        files,
    }
}

fn headers(patch: &str) -> Vec<&str> {
    patch
        .lines()
        .filter(|line| line.starts_with("diff --git "))
        .collect()
}

#[tokio::test]
async fn a_tracked_edit_and_an_untracked_addition_both_read_as_ordinary_diffs() {
    let scene = scene().await;
    for (name, needle) in [
        ("tracked.txt", "+two\n"),
        ("fresh.txt", "+brand new\n"),
        ("staged.txt", "+staged\n"),
    ] {
        let file = scene
            .files
            .iter()
            .find(|file| file.path.as_str() == name)
            .unwrap();
        let patch = patch_for(&scene.repo, &scene.sides, file, 3, None)
            .await
            .unwrap();
        assert!(
            patch.starts_with(&format!("diff --git a/{name} b/{name}\n")),
            "{patch}"
        );
        assert!(patch.contains(needle), "{patch}");
    }
}

#[tokio::test]
async fn patching_everything_matches_patching_the_files_one_by_one() {
    let scene = scene().await;
    let all = patch_all(&scene.repo, &scene.sides, &scene.files, 3)
        .await
        .unwrap();
    let mut names = headers(&all);
    names.sort_unstable();
    assert_eq!(
        names,
        [
            "diff --git a/fresh.txt b/fresh.txt",
            "diff --git a/other.txt b/other.txt",
            "diff --git a/staged.txt b/staged.txt",
            "diff --git a/tracked.txt b/tracked.txt",
        ]
    );
    let refs: Vec<&ChangedFile> = scene.files.iter().collect();
    let many = patch_many(&scene.repo, &scene.sides, &refs, 3)
        .await
        .unwrap();
    let mut many_names = headers(&many);
    many_names.sort_unstable();
    assert_eq!(many_names, names);
}

#[tokio::test]
async fn patch_many_names_both_sides_of_a_rename() {
    let fixture = TestRepo::init();
    let base = fixture.commit(
        "old.txt",
        "a body long enough\nto count as the same\nfile after a rename\n",
    );
    fixture.git(&["mv", "old.txt", "new.txt"]);
    fixture.git(&["commit", "--quiet", "--message", "rename"]);
    let tip = TreeSource::Object(
        crate::domains::repository::names::object_id::ObjectId::parse(
            &fixture.git(&["rev-parse", "HEAD"]),
        )
        .unwrap(),
    );
    let repo = GitRepo::open(fixture.path()).await.unwrap();
    let old = TreeSource::Object(base);
    let files = changed_files(&repo, &old, &tip).await.unwrap();
    let sides = DiffSides::new(&old, &tip);
    let refs: Vec<&ChangedFile> = files.iter().collect();
    let patch = patch_many(&repo, &sides, &refs, 3).await.unwrap();
    assert!(
        patch.contains("rename from old.txt\nrename to new.txt"),
        "{patch}"
    );
}

#[tokio::test]
async fn the_worktree_against_itself_has_no_patch() {
    let scene = scene().await;
    let nothing = DiffSides::new(&TreeSource::Worktree, &TreeSource::Worktree);
    assert_eq!(
        patch_all(&scene.repo, &nothing, &scene.files, 3)
            .await
            .unwrap(),
        ""
    );
    let refs: Vec<&ChangedFile> = scene.files.iter().collect();
    assert_eq!(
        patch_many(&scene.repo, &nothing, &refs, 3).await.unwrap(),
        ""
    );
}
