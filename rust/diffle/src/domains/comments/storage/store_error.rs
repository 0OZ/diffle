//! What can go wrong keeping comments on disk.

use std::io;
use std::path::PathBuf;

use thiserror::Error;

use super::versions_store::StoredDataError;
use crate::domains::comments::comment_error::{NotFound, Unquotable};
use crate::kernel::file_lock::LockError;

/// Anything a store operation can fail with. [`NotFound`] and [`Unquotable`] are the caller's
/// fault (the HTTP layer answers 404 and 400); the rest are the machine's or the file's.
#[derive(Debug, Error)]
pub enum CommentError {
    #[error(transparent)]
    NotFound(#[from] NotFound),
    #[error(transparent)]
    Unquotable(#[from] Unquotable),
    #[error(transparent)]
    Lock(#[from] LockError),
    #[error("cannot access {}: {source}", path.display())]
    Io {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("malformed {}: {source}", path.display())]
    Malformed {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
    #[error("invalid comment data in {}: {source}", path.display())]
    Invalid {
        path: PathBuf,
        #[source]
        source: StoredDataError,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_domain_refusal_reads_as_itself() {
        let id = crate::domains::comments::threads::ids::ThreadId::from("t1".to_owned());
        let error = CommentError::from(NotFound::Thread(id));
        assert_eq!(error.to_string(), "no thread t1");
        let io = CommentError::Io {
            path: PathBuf::from("/x/comments.json"),
            source: io::Error::other("disk full"),
        };
        assert_eq!(io.to_string(), "cannot access /x/comments.json: disk full");
    }
}
