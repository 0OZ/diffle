use std::path::PathBuf;

use super::*;
use crate::domains::revisions::revspec_error::ResolveError;

#[test]
fn input_the_reviewer_can_fix_exits_2() {
    let unknown = ReviewError::Resolve(ResolveError::Revspec(RevspecError::RangeWithTwoRevisions));
    assert_eq!(AppError::from(unknown).exit_status(), 2);
    assert_eq!(
        AppError::from(RevspecError::RangeWithTwoRevisions).exit_status(),
        2
    );
    assert_eq!(AppError::from(ReviewError::NothingToFocus).exit_status(), 2);
    let taken = ServerError::PortTaken {
        host: "127.0.0.1".to_owned(),
        port: 4966,
    };
    assert_eq!(AppError::from(taken).exit_status(), 2);
}

#[test]
fn what_the_program_or_machine_got_wrong_exits_1() {
    assert_eq!(AppError::from(ReviewError::NotOpen).exit_status(), 1);
    let no_build = ServerError::NoClientBuild {
        dir: PathBuf::from("dist/client"),
    };
    assert_eq!(AppError::from(no_build).exit_status(), 1);
    assert_eq!(
        AppError::from(ConfigError::NoConfigDirectory).exit_status(),
        1
    );
    let signals = AppError::Signals(io::Error::other("no handler"));
    assert_eq!(signals.exit_status(), 1);
}

#[test]
fn the_message_is_the_cause_without_a_prefix() {
    let error = AppError::from(ServerError::NoFreePort {
        host: "h".to_owned(),
        first: 1,
        last: 2,
    });
    assert_eq!(
        error.to_string(),
        "no free port from 1 to 2 on h; free one or pass --port"
    );
}
