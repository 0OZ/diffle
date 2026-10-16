//! `git grep` over a commit or the checkout, stopped as soon as it has found enough.

use std::collections::HashMap;
use std::time::Duration;

use super::line_span::LineSpan;
use crate::domains::repository::git_repo::GitRepo;
use crate::domains::repository::names::repo_path::RepoPath;
use crate::domains::repository::names::tree_source::TreeSource;
use crate::domains::repository::process::git_error::GitError;
use crate::domains::repository::process::git_runner::Exit;
use crate::domains::repository::process::git_stream::RecordStream;

/// A search that runs this long is a runaway regex, not a slow repository.
const GREP_TIMEOUT: Duration = Duration::from_secs(10);
/// A match's text is cut here, so one minified line cannot bloat a response.
const MAX_TEXT_CHARS: usize = 300;
/// `-z` prints `path NUL line NUL text NEWLINE`; the newline ends a record after two fields.
const RECORD_TERMINATOR: u8 = b'\n';
const LEADING_FIELDS: usize = 2;
/// With a commit, `git grep` names files `<commit>:<path>`.
const REVISION_SEPARATOR: char = ':';

/// What to search for besides the query; the default is a case-sensitive fixed string
/// anywhere in the tree.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GrepOptions {
    /// Match whole words only.
    pub word: bool,
    pub ignore_case: bool,
    /// Read the query as an extended regular expression instead of a fixed string.
    pub regex: bool,
    /// Search only these files. `Some` of none searches nothing.
    pub paths: Option<Vec<RepoPath>>,
    /// Keep only matches on these new-side line spans, before the limit applies. `Some` of
    /// none searches nothing.
    pub ranges: Option<HashMap<RepoPath, Vec<LineSpan>>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchMatch {
    pub path: RepoPath,
    /// 1-based.
    pub line: u64,
    /// The line, cut to its first 300 characters.
    pub text: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SearchHits {
    pub matches: Vec<SearchMatch>,
    /// More matches exist than `limit`.
    pub truncated: bool,
}

impl GitRepo {
    /// Searches `source` for `query`: at most `limit` matches, and `truncated` says there were
    /// more. Binary files are skipped, and the worktree includes untracked files. Git is
    /// stopped once `limit + 1` matches have arrived, so a common query in a huge repository
    /// cannot flood memory. An empty query, path list or range map matches nothing without
    /// starting Git. A search that outlives its budget fails with [`GitError::TimedOut`]
    /// and returns none of what it found. A match on a path that is no [`RepoPath`] is dropped.
    pub async fn grep(
        &self,
        query: &str,
        source: &TreeSource,
        limit: usize,
        options: &GrepOptions,
    ) -> Result<SearchHits, GitError> {
        let nothing_to_search = query.is_empty()
            || options.paths.as_ref().is_some_and(Vec::is_empty)
            || options.ranges.as_ref().is_some_and(HashMap::is_empty);
        if nothing_to_search {
            return Ok(SearchHits::default());
        }
        let spec = RecordStream {
            terminator: RECORD_TERMINATOR,
            leading_fields: LEADING_FIELDS,
            max_records: limit.saturating_add(1),
            timeout: GREP_TIMEOUT,
            // Git answers "nothing matched" with 1.
            accepted_exits: &[Exit::NO_ANSWER],
        };
        let mut matches = self
            .runner
            .stream_records(grep_args(query, source, limit, options), &spec, |record| {
                let found = parse_record(record, source)?;
                within_ranges(&found, options.ranges.as_ref()).then_some(found)
            })
            .await?;
        let truncated = matches.len() > limit;
        matches.truncate(limit);
        Ok(SearchHits { matches, truncated })
    }
}

fn grep_args(query: &str, source: &TreeSource, limit: usize, options: &GrepOptions) -> Vec<String> {
    // `--no-column`: `grep.column=true` would splice a column field into the `-z` record.
    let mut args = ["grep", "-n", "--no-column", "-I"]
        .map(str::to_owned)
        .to_vec();
    args.push(if options.regex { "-E" } else { "-F" }.to_owned());
    args.push("-z".to_owned());
    // Per file, so it only helps while the first matches are the wanted ones. A range filter
    // would otherwise let early matches outside the hunks hide later visible hits.
    if options.ranges.is_none() {
        args.push(format!("--max-count={}", limit.saturating_add(1)));
    }
    if options.word {
        args.push("-w".to_owned());
    }
    if options.ignore_case {
        args.push("-i".to_owned());
    }
    args.extend(["-e".to_owned(), query.to_owned()]);
    args.push(match source {
        TreeSource::Worktree => "--untracked".to_owned(),
        TreeSource::Object(id) => id.to_string(),
    });
    args.push("--".to_owned());
    // Literal pathspecs: a path with `*` or `?` in it must not turn into a glob.
    for path in options.paths.iter().flatten() {
        args.push(format!(":(literal){path}"));
    }
    args
}

fn parse_record(record: &[u8], source: &TreeSource) -> Option<SearchMatch> {
    let mut fields = record.splitn(LEADING_FIELDS + 1, |&byte| byte == 0);
    let (raw_path, line, text) = (fields.next()?, fields.next()?, fields.next()?);
    let raw_path = String::from_utf8_lossy(raw_path);
    let path = match source {
        TreeSource::Worktree => raw_path.as_ref(),
        TreeSource::Object(_) => raw_path
            .split_once(REVISION_SEPARATOR)
            .map_or(raw_path.as_ref(), |(_, path)| path),
    };
    Some(SearchMatch {
        path: RepoPath::parse(path).ok()?,
        line: std::str::from_utf8(line).ok()?.parse().ok()?,
        text: String::from_utf8_lossy(text)
            .chars()
            .take(MAX_TEXT_CHARS)
            .collect(),
    })
}

fn within_ranges(found: &SearchMatch, ranges: Option<&HashMap<RepoPath, Vec<LineSpan>>>) -> bool {
    ranges.is_none_or(|ranges| {
        ranges
            .get(&found.path)
            .is_some_and(|spans| spans.iter().any(|span| span.contains(found.line)))
    })
}

#[cfg(test)]
#[path = "grep_tests.rs"]
mod tests;
