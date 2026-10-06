use serde::Deserialize;

use crate::token_source::{self, KeyValueTokenSource};

#[derive(Debug, Deserialize, Clone)]
pub enum TokenSourceMethodConfig {
    #[serde(rename = "iam")]
    Iam { project_id: String },

    #[serde(rename = "fixed_token")]
    FixedToken { token: String },
}

impl Default for TokenSourceMethodConfig {
    fn default() -> Self {
        TokenSourceMethodConfig::FixedToken {
            token: "development_password".to_string(),
        }
    }
}

pub async fn get_token_source(
    auth_method: &TokenSourceMethodConfig,
) -> Result<Box<dyn KeyValueTokenSource + Send + Sync>, token_source::TokenSourceError> {
    match auth_method {
        TokenSourceMethodConfig::Iam { project_id } => {
            let src = token_source::KeyValueIAMTokenSource::new(project_id).map_err(|e| {
                token_source::TokenSourceError::UnableToCreateTokenSource(e.to_string())
            })?;
            Ok(Box::new(src))
        }
        TokenSourceMethodConfig::FixedToken { token } => Ok(Box::new(
            token_source::KeyValueStringTokenSource::new(token.clone()),
        )),
    }
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct TokenSourceConfig {
    #[serde(default, flatten)]
    pub method: TokenSourceMethodConfig,
}

#[derive(Debug, Deserialize, Clone)]
pub struct KeyValueStoreConfig {
    #[serde(default)]
    pub host: String,

    #[serde(default = "redis_port")]
    pub port: u16,

    // Number of retries to perform when a connection fails.
    #[serde(default = "number_of_retries")]
    pub number_of_retries: usize,

    #[serde(default)]
    pub auth: TokenSourceConfig,

    #[serde(default)]
    pub cluster_mode_enabled: bool,

    #[serde(default)]
    pub request_timeout: Option<u32>,

    #[serde(default = "connection_timeout")]
    pub connection_timeout: u32,
}

impl Default for KeyValueStoreConfig {
    fn default() -> Self {
        Self {
            host: "localhost".to_string(),
            port: redis_port(),
            number_of_retries: number_of_retries(),
            auth: TokenSourceConfig::default(),
            cluster_mode_enabled: false,
            request_timeout: None,
            connection_timeout: connection_timeout(),
        }
    }
}

const fn redis_port() -> u16 {
    6379 // Default Redis port
}

const fn number_of_retries() -> usize {
    6
}

const fn connection_timeout() -> u32 {
    5000
}
