//! Reading a request: a bounded JSON body and a typed query string, each refusing with the
//! API's own error instead of the framework's.

use axum::body::Body;
use axum::extract::{FromRequest, FromRequestParts, Query, Request};
use axum::http::request::Parts;
use futures::StreamExt;
use serde::de::DeserializeOwned;

use super::error_api::ApiError;

/// JSON bodies are small (comments, viewed marks, config); anything bigger is not a client.
pub const MAX_BODY_BYTES: usize = 1024 * 1024;
/// How much of an oversized upload is read and thrown away before answering: closing while the
/// client is still writing makes it see a reset instead of the 413.
const MAX_DRAINED_BYTES: usize = 64 * 1024 * 1024;

/// A JSON body, whatever its `Content-Type`.
pub struct JsonBody<T>(pub T);

impl<S, T> FromRequest<S> for JsonBody<T>
where
    S: Send + Sync,
    T: DeserializeOwned,
{
    type Rejection = ApiError;

    async fn from_request(request: Request, _state: &S) -> Result<Self, Self::Rejection> {
        let bytes = read_bounded(request.into_body()).await?;
        // Syntax errors and shape errors are told apart: the first is not the schema's fault.
        let value: serde_json::Value = serde_json::from_slice(&bytes)
            .map_err(|_| ApiError::bad_request("invalid JSON body"))?;
        serde_json::from_value(value)
            .map(Self)
            .map_err(|error| ApiError::bad_request(error.to_string()))
    }
}

/// The whole body, if it fits [`MAX_BODY_BYTES`]; an oversized one is drained and refused.
async fn read_bounded(body: Body) -> Result<Vec<u8>, ApiError> {
    let mut chunks = body.into_data_stream();
    let mut bytes = Vec::new();
    let mut seen = 0_usize;
    while let Some(chunk) = chunks.next().await {
        let chunk = chunk.map_err(|_| ApiError::bad_request("could not read the request body"))?;
        seen = seen.saturating_add(chunk.len());
        match seen <= MAX_BODY_BYTES {
            true => bytes.extend_from_slice(&chunk),
            false => bytes = Vec::new(),
        }
        if seen > MAX_DRAINED_BYTES {
            break;
        }
    }
    match seen <= MAX_BODY_BYTES {
        true => Ok(bytes),
        false => Err(ApiError::TooLarge),
    }
}

/// The query string as `T`.
pub struct ApiQuery<T>(pub T);

impl<S, T> FromRequestParts<S> for ApiQuery<T>
where
    S: Send + Sync,
    T: DeserializeOwned,
{
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        Query::<T>::try_from_uri(&parts.uri)
            .map(|Query(query)| Self(query))
            .map_err(|error| ApiError::bad_request(error.to_string()))
    }
}

#[cfg(test)]
#[path = "input_api_tests.rs"]
mod tests;
