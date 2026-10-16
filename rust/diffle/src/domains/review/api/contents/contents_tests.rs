//! Patches, files, pictures and search over real routes and a real repository.

use axum::http::{Method, StatusCode, header};
use serde_json::json;

use crate::domains::review::api::api_fixtures::Fixture;

const PNG: &[u8] = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR";

/// The commit that adds `b.txt`, shown against its parent.
async fn shown_commit() -> Fixture {
    Fixture::opened(&["HEAD^!"]).await
}

#[tokio::test]
async fn a_patch_is_served_for_one_file_for_all_or_for_a_batch() {
    let fixture = shown_commit().await;
    let one = fixture.get("/api/patch?path=b.txt").await;
    assert_eq!(one.status, StatusCode::OK);
    assert!(
        one.text().contains("diff --git a/b.txt b/b.txt"),
        "{}",
        one.text()
    );
    assert!(one.text().contains("+b"), "{}", one.text());
    assert_eq!(fixture.get("/api/patch").await.text(), one.text());

    let batch = fixture
        .send(
            Method::POST,
            "/api/patch",
            json!({"paths": ["b.txt", "nope.txt", "../x"]}),
        )
        .await;
    assert_eq!(batch.text(), one.text());
}

#[tokio::test]
async fn a_patch_of_an_unchanged_or_unknown_path_is_not_found() {
    let fixture = shown_commit().await;
    for path in ["a.txt", "nope.txt", "../a.txt"] {
        let answer = fixture.get(&format!("/api/patch?path={path}")).await;
        assert_eq!(answer.status, StatusCode::NOT_FOUND, "{path}");
        assert_eq!(answer.json(), json!({"error": "not a changed file"}));
    }
}

#[tokio::test]
async fn a_batch_of_patches_has_a_ceiling() {
    let fixture = shown_commit().await;
    let paths: Vec<String> = (0..201).map(|n| format!("f{n}.txt")).collect();
    let answer = fixture
        .send(Method::POST, "/api/patch", json!({ "paths": paths }))
        .await;
    assert_eq!(answer.status, StatusCode::BAD_REQUEST);
    assert_eq!(
        answer.json(),
        json!({"error": "at most 200 paths per request"})
    );
}

#[tokio::test]
async fn a_file_is_read_from_the_side_asked_for() {
    let fixture = shown_commit().await;
    let new = fixture.get("/api/file?path=b.txt&rev=new").await;
    assert_eq!(
        new.json(),
        json!({"path": "b.txt", "contents": "b\n", "binary": false})
    );
    let unchanged = fixture.get("/api/file?path=a.txt&rev=old").await;
    assert_eq!(unchanged.json()["contents"], json!("a\n"));
}

#[tokio::test]
async fn a_file_that_is_not_on_that_side_is_absent() {
    let fixture = shown_commit().await;
    for uri in [
        "/api/file?path=b.txt&rev=old",
        "/api/file?path=nope.txt&rev=new",
        "/api/file?path=../a.txt&rev=new",
        "/api/file?path=.git/config&rev=new",
    ] {
        let answer = fixture.get(uri).await;
        assert_eq!(answer.status, StatusCode::NOT_FOUND, "{uri}");
        assert_eq!(
            answer.json(),
            json!({"error": "absent on that side"}),
            "{uri}"
        );
    }
}

#[tokio::test]
async fn a_file_request_must_name_a_path_and_a_side() {
    let fixture = shown_commit().await;
    assert_eq!(
        fixture.get("/api/file?path=&rev=new").await.status,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        fixture.get("/api/file?path=a.txt").await.status,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        fixture.get("/api/file?path=a.txt&rev=middle").await.status,
        StatusCode::BAD_REQUEST
    );
}

#[tokio::test]
async fn a_binary_file_has_no_contents_to_show() {
    let fixture = Fixture::two_commits().await;
    std::fs::write(fixture.repo.path().join("blob.bin"), b"a\0b").unwrap();
    fixture.repo.git(&["add", "--", "blob.bin"]);
    fixture
        .repo
        .git(&["commit", "--quiet", "--message", "binary"]);
    fixture
        .session
        .start(fixture.comparison(&["HEAD^!"]).await)
        .await
        .unwrap();
    let answer = fixture.get("/api/file?path=blob.bin&rev=new").await;
    assert_eq!(
        answer.json(),
        json!({"path": "blob.bin", "contents": "", "binary": true})
    );
}

async fn with_picture() -> (Fixture, String) {
    let fixture = Fixture::two_commits().await;
    std::fs::write(fixture.repo.path().join("logo.png"), PNG).unwrap();
    fixture.repo.git(&["add", "--", "logo.png"]);
    fixture
        .repo
        .git(&["commit", "--quiet", "--message", "logo"]);
    fixture
        .session
        .start(fixture.comparison(&["HEAD^!"]).await)
        .await
        .unwrap();
    let key = fixture.repo.git(&["rev-parse", "HEAD:logo.png"]);
    (fixture, key)
}

#[tokio::test]
async fn a_picture_is_served_under_its_key_and_cached_for_good() {
    let (fixture, key) = with_picture().await;
    let answer = fixture
        .get(&format!("/api/image?path=logo.png&rev=new&key={key}"))
        .await;
    assert_eq!(answer.status, StatusCode::OK);
    assert_eq!(answer.body, PNG);
    let header = |name: &str| answer.headers[name].to_str().unwrap().to_owned();
    assert_eq!(header("content-type"), "image/png");
    assert_eq!(
        header("cache-control"),
        "private, max-age=31536000, immutable"
    );
    assert_eq!(header("x-content-type-options"), "nosniff");
    assert_eq!(header("cross-origin-resource-policy"), "same-origin");
}

#[tokio::test]
async fn a_picture_under_any_other_key_is_not_found() {
    let (fixture, _) = with_picture().await;
    let answer = fixture
        .get("/api/image?path=logo.png&rev=new&key=nope")
        .await;
    assert_eq!(answer.status, StatusCode::NOT_FOUND);
    assert_eq!(answer.json(), json!({"error": "no image under that key"}));
    assert!(answer.headers.get(header::CACHE_CONTROL).is_none());
}

async fn found(fixture: &Fixture, query: &str) -> Vec<(String, u64, String)> {
    let answer = fixture.get(&format!("/api/search?{query}")).await;
    assert_eq!(answer.status, StatusCode::OK, "{}", answer.text());
    answer.json()["matches"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| {
            (
                m["path"].as_str().unwrap().to_owned(),
                m["line"].as_u64().unwrap(),
                m["text"].as_str().unwrap().to_owned(),
            )
        })
        .collect()
}

#[tokio::test]
async fn search_defaults_to_the_lines_the_diff_shows() {
    let fixture = shown_commit().await;
    assert_eq!(
        found(&fixture, "q=b").await,
        [("b.txt".to_owned(), 1, "b".to_owned())]
    );
    assert!(
        found(&fixture, "q=a").await.is_empty(),
        "a.txt is not in the diff"
    );
    let answer = fixture.get("/api/search?q=b").await.json();
    assert_eq!(answer["query"], json!("b"));
    assert_eq!(answer["truncated"], json!(false));
}

#[tokio::test]
async fn search_widens_to_whole_files_or_the_whole_tree() {
    let fixture = shown_commit().await;
    assert!(
        found(&fixture, "q=a&scope=diff&content=full")
            .await
            .is_empty()
    );
    assert_eq!(
        found(&fixture, "q=a&scope=repo&content=full").await,
        [("a.txt".to_owned(), 1, "a".to_owned())]
    );
    assert!(
        found(&fixture, "q=a&scope=repo&content=diff")
            .await
            .is_empty()
    );
    assert_eq!(
        found(&fixture, "q=a&scope=file&path=a.txt&content=full")
            .await
            .len(),
        1
    );
    assert!(
        found(&fixture, "q=a&scope=file&path=nope.txt&content=full")
            .await
            .is_empty()
    );
}

#[tokio::test]
async fn search_flags_are_switches_spelled_one() {
    let fixture = shown_commit().await;
    assert!(found(&fixture, "q=B").await.is_empty());
    assert_eq!(found(&fixture, "q=B&i=1").await.len(), 1);
    assert!(
        found(&fixture, "q=%5Eb%24").await.is_empty(),
        "no regex without re=1"
    );
    assert_eq!(found(&fixture, "q=%5Eb%24&re=1").await.len(), 1);
    assert_eq!(found(&fixture, "q=b&word=1").await.len(), 1);
    assert!(found(&fixture, "q=").await.is_empty());
}

#[tokio::test]
async fn search_refuses_a_scope_it_does_not_have() {
    let fixture = shown_commit().await;
    assert_eq!(
        fixture.get("/api/search?q=b&scope=galaxy").await.status,
        StatusCode::BAD_REQUEST
    );
}
