//! What the route tests share: a real repository, a session over it, and a way to call routes.

use std::sync::Arc;

use axum::Router;
use axum::body::{Body, to_bytes};
use axum::http::{HeaderMap, Method, Request, StatusCode};
use serde::Serialize;
use serde_json::Value;
use tower::ServiceExt;

use super::api_state::ApiState;
use super::router_api::router;
use crate::domains::repository::git_repo::GitRepo;
use crate::domains::repository::names::object_id::ObjectId;
use crate::domains::repository::test_repo::TestRepo;
use crate::domains::review::session::review_session::{Session, SessionConfig};
use crate::domains::revisions::comparison::Comparison;
use crate::domains::revisions::entry_request::EntryRequest;
use crate::domains::revisions::revspec::Revspec;
use crate::domains::settings::config_store::ConfigStore;
use crate::domains::settings::context_lines::ContextLines;
use crate::domains::settings::follow_refs::FollowRefs;

/// What a route answered.
pub struct Answer {
    pub status: StatusCode,
    pub headers: HeaderMap,
    pub body: Vec<u8>,
}

impl Answer {
    pub fn text(&self) -> String {
        String::from_utf8_lossy(&self.body).into_owned()
    }

    pub fn json(&self) -> Value {
        serde_json::from_slice(&self.body)
            .unwrap_or_else(|error| panic!("not JSON ({error}): {}", self.text()))
    }
}

/// `a.txt` in a first commit and `b.txt` in the second, a session over them that has not been
/// started yet, and a config store of its own.
pub struct Fixture {
    pub repo: TestRepo,
    pub session: Arc<Session>,
    pub state: ApiState,
    parent: ObjectId,
    head: ObjectId,
    _config_dir: tempfile::TempDir,
}

impl Fixture {
    pub async fn two_commits() -> Self {
        let repo = TestRepo::init();
        let parent = repo.commit("a.txt", "a\n");
        let head = repo.commit("b.txt", "b\n");
        let git = Arc::new(GitRepo::open(repo.path()).await.unwrap());
        let session = Session::new(
            git,
            SessionConfig {
                context: ContextLines::default(),
                follow_refs: FollowRefs::default(),
                watcher: None,
            },
        );
        let config_dir = tempfile::tempdir().unwrap();
        let config = ConfigStore::open(config_dir.path().join("config.json"))
            .await
            .unwrap();
        let state = ApiState::new(Arc::clone(&session), Arc::new(config), Vec::new());
        Self {
            repo,
            session,
            state,
            parent,
            head,
            _config_dir: config_dir,
        }
    }

    /// Like [`two_commits`](Self::two_commits), with the session open on the first request.
    pub async fn opened(args: &[&str]) -> Self {
        let fixture = Self::two_commits().await;
        fixture
            .session
            .start(fixture.comparison(args).await)
            .await
            .unwrap();
        fixture
    }

    /// No arguments is the working review; otherwise the revspec they spell.
    pub async fn comparison(&self, args: &[&str]) -> Comparison {
        self.session.resolve(&request(args)).await.unwrap()
    }

    pub fn head(&self) -> &str {
        self.head.as_str()
    }

    pub fn parent(&self) -> &str {
        self.parent.as_str()
    }

    pub fn root(&self) -> String {
        self.session.repo().root().to_string_lossy().into_owned()
    }

    pub fn router(&self) -> Router {
        router(self.state.clone())
    }

    pub async fn get(&self, uri: &str) -> Answer {
        self.call(Method::GET, uri, None).await
    }

    pub async fn send(&self, method: Method, uri: &str, body: Value) -> Answer {
        self.call(method, uri, Some(body)).await
    }

    pub async fn delete(&self, uri: &str) -> Answer {
        self.call(Method::DELETE, uri, None).await
    }

    async fn call(&self, method: Method, uri: &str, body: Option<Value>) -> Answer {
        let body = body.map_or_else(Body::empty, |body| Body::from(body.to_string()));
        let request = Request::builder()
            .method(method)
            .uri(uri)
            .body(body)
            .unwrap();
        let response = self.router().oneshot(request).await.unwrap();
        let (parts, body) = response.into_parts();
        Answer {
            status: parts.status,
            headers: parts.headers,
            body: to_bytes(body, usize::MAX).await.unwrap().to_vec(),
        }
    }
}

fn request(args: &[&str]) -> EntryRequest {
    match args.is_empty() {
        true => EntryRequest::Working,
        false => {
            let owned: Vec<String> = args.iter().map(|arg| (*arg).to_owned()).collect();
            EntryRequest::Revspec(Revspec::parse(&owned).unwrap())
        }
    }
}

pub fn json_of<T: Serialize>(value: &T) -> Value {
    serde_json::to_value(value).unwrap()
}
