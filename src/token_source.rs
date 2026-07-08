use google_cloud_auth::{build_errors, credentials::CacheableResource};
use thiserror::Error;
const AUTHORIZATION: &str = "Authorization";

#[derive(Error, Debug)]
pub enum TokenSourceError {
    #[error("Failed to get token: {0}")]
    GetTokenError(String),

    #[error("Failed to create token source: {0}")]
    UnableToCreateTokenSource(String),
}

// The way KeyValueStore works is by recreating the connection manager
// from time to time with the password obtained from the token source.
// We need to implement this in order to fulfill requirements of the
// of Valkey AUTH for Google cloud.
#[async_trait::async_trait]
pub trait KeyValueTokenSource {
    async fn get_token(&self) -> Result<String, TokenSourceError>;
}

pub struct KeyValueIAMTokenSource {
    credentials: google_cloud_auth::credentials::Credentials,
}

#[async_trait::async_trait]
impl KeyValueTokenSource for KeyValueIAMTokenSource {
    // Google cloud Crate is still under development so the APIs can change
    // in the future.
    //
    // It seems that the only way to obtain a token is to use the
    // google_cloud_auth::credentials API, which provides a way to obtain
    // the authorization headers, which contain the token.
    async fn get_token(&self) -> Result<String, TokenSourceError> {
        // Create a token source with the given scopes.
        match self.credentials.headers(http::Extensions::new()).await {
            Ok(headers) => match Self::get_token_from_headers(&headers) {
                Some(token) => return Ok(token),
                None => {
                    return Err(TokenSourceError::GetTokenError(
                        "Failed to get token from headers".to_string(),
                    ))
                }
            },
            Err(e) => Err(TokenSourceError::GetTokenError(e.to_string())),
        }
    }
}

impl KeyValueIAMTokenSource {
    pub fn new(project_id: &str) -> Result<Self, build_errors::Error> {
        // Create a token source using the default credentials.
        let credentials = google_cloud_auth::credentials::Builder::default()
            .with_quota_project_id(project_id)
            .with_scopes(["https://www.googleapis.com/auth/cloud-platform"])
            .build()?;
        Ok(Self { credentials })
    }

    fn get_token_from_headers(headers: &CacheableResource<http::HeaderMap>) -> Option<String> {
        match headers {
            CacheableResource::NotModified => None,
            CacheableResource::New {
                entity_tag: _entity_tag,
                data,
            } => data
                .get(AUTHORIZATION)
                .and_then(|token_value| token_value.to_str().ok())
                .and_then(|s| s.split_whitespace().nth(1))
                .map(|s| s.to_string()),
        }
    }
}

// A a simple token source that returns a static string.
// To be used for testing purposes or when a static token is sufficient.
pub struct KeyValueStringTokenSource {
    token: String,
}

#[async_trait::async_trait]
impl KeyValueTokenSource for KeyValueStringTokenSource {
    async fn get_token(&self) -> Result<String, TokenSourceError> {
        Ok(self.token.clone())
    }
}

impl KeyValueStringTokenSource {
    pub fn new(token: String) -> Self {
        KeyValueStringTokenSource { token }
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    #[tokio::test]
    async fn test_key_value_string_token_source() {
        let token_source = KeyValueStringTokenSource::new("test-token".to_string());
        let token = token_source.get_token().await.unwrap();
        assert_eq!(token, "test-token");
    }
}
