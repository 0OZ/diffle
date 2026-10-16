use super::*;
use crate::domains::repository::test_repo::TestRepo;

const BUDGET: Duration = Duration::from_secs(20);

fn spec(max_records: usize, leading_fields: usize) -> RecordStream<'static> {
    RecordStream {
        terminator: b'\n',
        leading_fields,
        max_records,
        timeout: BUDGET,
        accepted_exits: &[],
    }
}

fn text(record: &[u8]) -> Option<String> {
    Some(String::from_utf8_lossy(record).into_owned())
}

#[tokio::test]
async fn cuts_records_at_the_terminator() {
    let fixture = TestRepo::init();
    let runner = GitRunner::new(fixture.path());
    fixture.git(&["config", "a.b", "one"]);
    fixture.git(&["config", "a.c", "two"]);
    let records = runner
        .stream_records(["config", "--list", "--local"], &spec(99, 0), text)
        .await
        .unwrap();
    assert!(records.iter().any(|record| record == "a.b=one"));
    assert!(records.iter().any(|record| record == "a.c=two"));
    assert!(records.iter().all(|record| !record.contains('\n')));
}

#[tokio::test]
async fn a_newline_inside_the_leading_fields_does_not_end_the_record() {
    let fixture = TestRepo::init();
    fixture.commit("odd\nname.txt", "needle\n");
    let runner = GitRunner::new(fixture.path());
    let args = ["grep", "-n", "-z", "needle", "HEAD", "--"];
    let records = runner
        .stream_records(args, &spec(9, 2), text)
        .await
        .unwrap();
    assert_eq!(records, ["HEAD:odd\nname.txt\x001\0needle"]);
}

#[tokio::test]
async fn the_filter_decides_what_is_kept_and_counts_toward_the_bound() {
    let fixture = TestRepo::init();
    fixture.commit("a.txt", "x1\nx2\nx3\nx4\n");
    let runner = GitRunner::new(fixture.path());
    let keep_even = |record: &[u8]| {
        let record = String::from_utf8_lossy(record).into_owned();
        record.ends_with(['2', '4']).then_some(record)
    };
    let args = ["grep", "-h", "x", "--", "a.txt"];
    let all = runner
        .stream_records(args, &spec(9, 0), keep_even)
        .await
        .unwrap();
    assert_eq!(all, ["x2", "x4"]);
    let one = runner
        .stream_records(args, &spec(1, 0), keep_even)
        .await
        .unwrap();
    assert_eq!(one, ["x2"]);
}

#[tokio::test]
async fn an_accepted_exit_is_an_answer_and_another_is_a_failure() {
    let fixture = TestRepo::init();
    fixture.commit("a.txt", "x\n");
    let runner = GitRunner::new(fixture.path());
    let args = ["grep", "absent", "--", "a.txt"];
    let failed = runner.stream_records(args, &spec(9, 0), text).await;
    assert!(matches!(
        failed,
        Err(GitError::Failed {
            exit: Exit::Failure(1),
            ..
        })
    ));
    let accepting = RecordStream {
        accepted_exits: &[Exit::NO_ANSWER],
        ..spec(9, 0)
    };
    let none = runner.stream_records(args, &accepting, text).await.unwrap();
    assert!(none.is_empty());
}

#[tokio::test]
async fn zero_records_runs_nothing() {
    let runner = GitRunner::new("/nonexistent/for/sure");
    let none = runner
        .stream_records(["status"], &spec(0, 0), text)
        .await
        .unwrap();
    assert!(none.is_empty());
}

// An alias runs a shell command in place of a Git one, which gives a source that never ends.
#[cfg(unix)]
#[tokio::test]
async fn stops_an_endless_command_once_enough_records_arrived() {
    let fixture = TestRepo::init();
    let runner = GitRunner::new(fixture.path());
    let args = ["-c", "alias.endless=!yes line", "endless"];
    let started = std::time::Instant::now();
    let records = runner
        .stream_records(args, &spec(3, 0), text)
        .await
        .unwrap();
    assert_eq!(records, ["line", "line", "line"]);
    assert!(started.elapsed() < BUDGET);
}

#[cfg(unix)]
#[tokio::test]
async fn kills_a_command_that_outlives_its_budget() {
    let fixture = TestRepo::init();
    let runner = GitRunner::new(fixture.path());
    let args = ["-c", "alias.slow=!sleep 5", "slow"];
    let hurried = RecordStream {
        timeout: Duration::from_millis(150),
        ..spec(1, 0)
    };
    let started = std::time::Instant::now();
    let result = runner.stream_records(args, &hurried, text).await;
    assert!(matches!(result, Err(GitError::TimedOut { .. })));
    assert!(started.elapsed() < Duration::from_secs(4));
}

#[cfg(unix)]
#[tokio::test]
async fn a_last_record_without_its_terminator_is_dropped() {
    let fixture = TestRepo::init();
    let runner = GitRunner::new(fixture.path());
    let args = ["-c", r"alias.cut=!printf 'whole\npart'", "cut"];
    let records = runner
        .stream_records(args, &spec(9, 0), text)
        .await
        .unwrap();
    assert_eq!(records, ["whole"]);
}
