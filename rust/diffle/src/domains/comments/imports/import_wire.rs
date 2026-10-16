//! The JSON `POST /api/threads` accepts: one object or an array of them (`ThreadCreateSchema`).
//!
//! The payload is validated by hand rather than derived, so a bad one is reported with the
//! words the TypeScript server's validator uses, field by field, in the same order.

use std::fmt;

use serde_json::{Map, Value};
use thiserror::Error;

use super::new_thread::{NewThread, ThreadTarget};
use crate::domains::comments::anchors::anchor::Quote;
use crate::domains::comments::anchors::lines::Line;
use crate::domains::comments::anchors::review_path::ReviewPath;
use crate::domains::comments::threads::body::{Body, BodyError, trimmed};
use crate::kernel::side::Side;

/// Everything `POST /api/threads` accepts: 1 MiB, like the HTTP body limit.
const MAX_PAYLOAD_BYTES: usize = 1024 * 1024;
/// The largest integer JavaScript represents exactly.
const MAX_SAFE_INTEGER: i128 = 9_007_199_254_740_991;

const PATH_TOO_SHORT: &str = "Too small: expected string to have >=1 characters";
const NOT_POSITIVE: &str = "Too small: expected number to be >0";
const TOO_BIG: &str = "Too big: expected int to be <=9007199254740991";
const NOT_AN_INT: &str = "Invalid input: expected int, received number";
const BAD_SIDE: &str = "Invalid option: expected one of \"old\"|\"new\"";
const END_BEFORE_START: &str = "endLine must be ≥ startLine";
const NEEDS_START_LINE: &str =
    "side, endLine and quoted need a startLine; leave all four out for a comment on the whole file";

/// A payload that is not valid JSON or not a thread request. The caller's fault.
#[derive(Debug, Error)]
pub enum ImportError {
    #[error("comment payload over 1 MiB")]
    TooLarge,
    #[error("invalid comment JSON: {0}")]
    Json(#[from] serde_json::Error),
    /// `index` names the entry when the payload held several.
    #[error("invalid comment{}: {}", entry_label(*.index), summary(.issues))]
    Invalid {
        index: Option<usize>,
        issues: Vec<Issue>,
    },
}

/// One thing wrong with one field; `field` is empty for the payload itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Issue {
    pub field: &'static str,
    pub message: String,
}

impl fmt::Display for Issue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.field, self.message)
    }
}

fn entry_label(index: Option<usize>) -> String {
    index.map_or_else(String::new, |index| format!("[{index}]"))
}

fn summary(issues: &[Issue]) -> String {
    issues
        .iter()
        .map(Issue::to_string)
        .collect::<Vec<_>>()
        .join("; ")
}

/// Parses a payload given as JSON text. Fails on any malformed entry, naming its index.
pub fn parse_imports(raw: &str) -> Result<Vec<NewThread>, ImportError> {
    if raw.len() > MAX_PAYLOAD_BYTES {
        return Err(ImportError::TooLarge);
    }
    parse_imports_value(serde_json::from_str(raw)?)
}

/// Parses a payload that is already a JSON value.
pub fn parse_imports_value(value: Value) -> Result<Vec<NewThread>, ImportError> {
    let entries = match value {
        Value::Array(entries) => entries,
        single @ (Value::Null
        | Value::Bool(_)
        | Value::Number(_)
        | Value::String(_)
        | Value::Object(_)) => vec![single],
    };
    let several = entries.len() > 1;
    entries
        .iter()
        .enumerate()
        .map(|(index, entry)| {
            validate(entry).map_err(|issues| ImportError::Invalid {
                index: several.then_some(index),
                issues,
            })
        })
        .collect()
}

#[derive(Default)]
struct Validation {
    issues: Vec<Issue>,
    /// A wrong type or option stops the cross-field checks, as in the original validator.
    aborted: bool,
}

impl Validation {
    fn flag(&mut self, field: &'static str, message: impl Into<String>) {
        self.issues.push(Issue {
            field,
            message: message.into(),
        });
    }

    fn abort(&mut self, field: &'static str, message: impl Into<String>) {
        self.aborted = true;
        self.flag(field, message);
    }

    fn string(
        &mut self,
        object: &Map<String, Value>,
        field: &'static str,
        required: bool,
    ) -> Option<String> {
        match object.get(field) {
            Some(Value::String(text)) => Some(text.clone()),
            None if !required => None,
            other => {
                self.abort(
                    field,
                    format!(
                        "Invalid input: expected string, received {}",
                        received(other)
                    ),
                );
                None
            }
        }
    }

    fn side(&mut self, object: &Map<String, Value>) -> Option<Side> {
        match object.get("side") {
            None => None,
            Some(Value::String(text)) if text == "old" => Some(Side::Old),
            Some(Value::String(text)) if text == "new" => Some(Side::New),
            Some(_) => {
                self.abort("side", BAD_SIDE);
                None
            }
        }
    }

    /// An optional positive integer; the raw value is kept even when out of range, for the
    /// cross-field checks.
    fn line(&mut self, object: &Map<String, Value>, field: &'static str) -> Option<i128> {
        let number = match object.get(field) {
            None => return None,
            Some(Value::Number(number)) => number,
            other => {
                self.abort(
                    field,
                    format!(
                        "Invalid input: expected number, received {}",
                        received(other)
                    ),
                );
                return None;
            }
        };
        let Some(value) = integer(number) else {
            self.abort(field, NOT_AN_INT);
            return None;
        };
        if value > MAX_SAFE_INTEGER {
            self.flag(field, TOO_BIG);
        } else if value < 1 {
            self.flag(field, NOT_POSITIVE);
        }
        Some(value)
    }
}

fn integer(number: &serde_json::Number) -> Option<i128> {
    if let Some(value) = number.as_i64() {
        return Some(i128::from(value));
    }
    if let Some(value) = number.as_u64() {
        return Some(i128::from(value));
    }
    number
        .as_f64()
        .filter(|value| value.is_finite() && value.fract() == 0.0)
        .map(|value| value as i128)
}

fn received(value: Option<&Value>) -> &'static str {
    match value {
        None => "undefined",
        Some(Value::Null) => "null",
        Some(Value::Bool(_)) => "boolean",
        Some(Value::Number(_)) => "number",
        Some(Value::String(_)) => "string",
        Some(Value::Array(_)) => "array",
        Some(Value::Object(_)) => "object",
    }
}

fn to_line(raw: Option<i128>) -> Option<Line> {
    raw.and_then(|value| u64::try_from(value).ok())
        .and_then(|value| Line::new(value).ok())
}

fn validate(entry: &Value) -> Result<NewThread, Vec<Issue>> {
    let Value::Object(object) = entry else {
        let message = format!(
            "Invalid input: expected object, received {}",
            received(Some(entry))
        );
        return Err(vec![Issue { field: "", message }]);
    };
    let mut check = Validation::default();
    let path = check.string(object, "path", true);
    if path.as_deref().is_some_and(str::is_empty) {
        check.flag("path", PATH_TOO_SHORT);
    }
    let side = check.side(object);
    let start = check.line(object, "startLine");
    let end = check.line(object, "endLine");
    let body = check.string(object, "body", true);
    if body.as_deref().is_some_and(|text| trimmed(text).is_empty()) {
        check.flag("body", BodyError::Blank.to_string());
    }
    let quoted = check.string(object, "quoted", false);
    if !check.aborted {
        if let (Some(start), Some(end)) = (start, end)
            && end < start
        {
            check.flag("endLine", END_BEFORE_START);
        }
        if start.is_none() && (side.is_some() || end.is_some() || quoted.is_some()) {
            check.flag("startLine", NEEDS_START_LINE);
        }
    }
    let (Some(path), Some(body), true) = (path, body, check.issues.is_empty()) else {
        return Err(check.issues);
    };
    let built = ReviewPath::parse(path)
        .map_err(|error| error.to_string())
        .and_then(|path| {
            let body = Body::parse(body).map_err(|error| error.to_string())?;
            let target = ThreadTarget::from_fields(
                to_line(start),
                to_line(end),
                side,
                quoted.map(Quote::new),
            )
            .map_err(|error| error.to_string())?;
            Ok(NewThread::new(path, target, body))
        });
    built.map_err(|message| vec![Issue { field: "", message }])
}

#[cfg(test)]
#[path = "import_wire_tests.rs"]
mod tests;
