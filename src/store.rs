use std::sync::Arc;

use arc_swap::ArcSwap;
use glide_core::{
    client::{AuthenticationInfo, Client, ConnectionRetryStrategy, NodeAddress, NodeDiscoveryMode},
    ConnectionRequest,
};

use thiserror::Error;
use tokio::task::JoinHandle;
use tokio_util::task::AbortOnDropHandle;

use crate::{config::get_token_source, token_source, KeyValueStoreConfig, KeyValueTokenSource};

pub const REFRESH_INTERVAL_SECS: u64 = 60; // 1 minute

#[derive(Error, Debug)]
pub enum KeyValueStoreError {
    #[error("Valkey error: {0}")]
    ValkeyError(#[from] glide_core::client::ConnectionError),

    #[error("Token source error: {0}")]
    TokenSourceError(#[from] token_source::TokenSourceError),
}

#[derive(Clone)]
pub struct KeyValueStore {
    client: Arc<ArcSwap<Client>>,
    _refresh_task: Arc<AbortOnDropHandle<()>>,
}

impl KeyValueStore {
    pub async fn new(config: &KeyValueStoreConfig) -> Result<Self, KeyValueStoreError> {
        let token_source = get_token_source(&config.auth.method).await?;
        Self::new_with_token_source(config, token_source).await
    }

    pub async fn new_with_token_source(
        config: &KeyValueStoreConfig,
        token_source: Box<dyn KeyValueTokenSource + Send + Sync>,
    ) -> Result<Self, KeyValueStoreError> {
        let token = token_source.get_token().await?;

        let client = Arc::new(ArcSwap::from_pointee(Self::connect(config, &token).await?));
        let _refresh_task = Arc::new(AbortOnDropHandle::new(
            Self::start_refresh_task(client.clone(), config, token_source, &token).await,
        ));

        let store = Self {
            client,
            _refresh_task,
        };

        Ok(store)
    }

    pub fn client(&self) -> Client {
        self.client.load().as_ref().clone()
    }

    async fn connect(
        config: &KeyValueStoreConfig,
        token: &str,
    ) -> Result<Client, glide_core::client::ConnectionError> {
        let request = Self::connection_request(config, token);
        Client::new(request, None).await
    }

    fn connection_request(config: &KeyValueStoreConfig, token: &str) -> ConnectionRequest {
        ConnectionRequest {
            addresses: vec![NodeAddress {
                host: config.host.clone(),
                port: config.port,
            }],
            connection_retry_strategy: Some(ConnectionRetryStrategy {
                exponent_base: 2,
                factor: 100,
                number_of_retries: config.number_of_retries as u32,
                jitter_percent: Some(20),
            }),
            authentication_info: Some(AuthenticationInfo {
                username: None,
                password: Some(token.to_string()),
                iam_config: None,
            }),
            cluster_mode_enabled: config.cluster_mode_enabled,
            request_timeout: config.request_timeout,
            refresh_topology_from_initial_nodes: false,
            // The current implementation uses TTL-based expiration only. There is no server-side invalidation
            // Which is unacceptable for OneSignal uses cases
            client_side_cache: None,
            read_from: None,
            client_name: None,
            lib_name: None,
            tcp_nodelay: true,
            root_certs: vec![],
            client_cert: vec![],
            client_key: vec![],
            database_id: 0,
            lazy_connect: false,
            read_only: false,
            node_discovery_mode: NodeDiscoveryMode::default(),
            protocol: None,
            tls_mode: None,
            connection_timeout: Some(config.connection_timeout),
            periodic_checks: None,
            pubsub_subscriptions: None,
            inflight_requests_limit: None,
            compression_config: None,
            pubsub_reconciliation_interval_ms: None,
            address_resolver: None,
        }
    }

    async fn start_refresh_task(
        client: Arc<ArcSwap<Client>>,
        config: &KeyValueStoreConfig,
        token_source: Box<dyn KeyValueTokenSource + Send + Sync + 'static>,
        initial_token: &str,
    ) -> JoinHandle<()> {
        let client = client.clone();
        let config = config.clone();
        let initial_token = initial_token.to_string();

        tokio::spawn(async move {
            let mut current_token = initial_token;

            loop {
                tracing::trace!(
                    duration = REFRESH_INTERVAL_SECS,
                    "Waiting before refreshing token",
                );
                tokio::time::sleep(tokio::time::Duration::from_secs(REFRESH_INTERVAL_SECS)).await;
                tracing::trace!("Refreshing connection manager with new token");

                let new_token = match token_source.get_token().await {
                    Ok(token) => token,
                    Err(error) => {
                        tracing::error!(
                            ?error,
                            "Failed to get new token, connections will shutdown after 12 hours"
                        );
                        continue;
                    }
                };

                if new_token == current_token {
                    continue;
                }
                tracing::info!("New token obtained, creating new connection manager");

                match Self::connect(&config, &new_token).await {
                    Ok(connection) => {
                        current_token = new_token;
                        client.store(Arc::new(connection));
                    }
                    Err(error) => {
                        tracing::error!(
                            ?error,
                            "Failed to create new connection, connections will shutdown after 12 hours"
                        );
                    }
                }
            }
        })
    }

    #[cfg(test)]
    pub async fn statistics(&self) -> crate::test_utils::CacheStatistics {
        let client = self.client();

        let hit: usize = redis::from_redis_value(&client.cache_hit_rate().unwrap()).unwrap();
        let miss: usize = redis::from_redis_value(&client.cache_miss_rate().unwrap()).unwrap();
        let invalidate: usize =
            redis::from_redis_value(&client.cache_evictions().unwrap()).unwrap();

        crate::test_utils::CacheStatistics {
            hit,
            miss,
            invalidate,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use redis::PipelineRetryStrategy;
    use serial_test::serial;

    use super::*;
    use crate::config::{KeyValueStoreConfig, TokenSourceMethodConfig};
    use crate::{test_utils::*, RedisExtensions};

    #[should_panic(expected = "WRONGPASS: invalid username-password pair or user is disabled")]
    #[tokio::test]
    #[serial]
    async fn test_new_fails_with_invalid_password() {
        let mut config = test_config();
        config.number_of_retries = 0;
        config.auth.method = TokenSourceMethodConfig::FixedToken {
            token: "invalid_password".to_string(),
        };

        KeyValueStore::new(&config).await.unwrap();
    }

    #[tokio::test]
    #[serial]
    async fn test_exists_returns_false_for_nonexistent_key() {
        let store = key_value_store().await;
        let mut client = store.client();

        let test_key = random_key();
        client.del(&test_key).await.unwrap();

        let exists: bool = client
            .exists(&test_key)
            .await
            .expect("Failed to check existence");
        assert!(!exists);
    }

    #[tokio::test]
    #[serial]
    async fn test_exists_returns_true_for_existing_key() {
        let store = key_value_store().await;
        let mut client = store.client();

        let test_key = &random_key();
        let test_value = "";

        client
            .set_ex(test_key, test_value, 10)
            .await
            .expect("Failed to set key");

        let exists = client
            .exists(test_key)
            .await
            .expect("Failed to check existence");
        assert!(exists);

        client.del(test_key).await.unwrap();
    }

    #[tokio::test]
    #[serial]
    async fn test_set_and_delete_operations() {
        let store = key_value_store().await;
        let mut client = store.client();

        let test_key = &random_key();
        let test_value = "test_value";

        client
            .set_ex(test_key, test_value, 10)
            .await
            .expect("Failed to set key");

        let exists_after_set = client
            .exists(test_key)
            .await
            .expect("Failed to check existence after set");
        assert!(exists_after_set);

        client.del(test_key).await.expect("Failed to delete key");

        let exists_after_delete = client
            .exists(test_key)
            .await
            .expect("Failed to check existence after delete");
        assert!(!exists_after_delete);
    }

    #[tokio::test]
    #[serial]
    async fn test_multiple_operations_with_different_keys() {
        let store = key_value_store().await;
        let mut client = store.client();

        let key1 = &random_key();
        let key2 = &random_key();

        client
            .set_ex(key1, "", 10)
            .await
            .expect("Failed to set key1");

        client
            .set_ex(key2, "", 10)
            .await
            .expect("Failed to set key2");

        let exists1 = client
            .exists(key1)
            .await
            .expect("Failed to check key1 existence");

        let exists2 = client
            .exists(key2)
            .await
            .expect("Failed to check key2 existence");

        assert!(exists1);
        assert!(exists2);

        client.del(key1).await.expect("Failed to delete key1");

        let exists1_after_delete = client
            .exists(key1)
            .await
            .expect("Failed to check key1 existence after delete");
        let exists2_after_delete = client
            .exists(key2)
            .await
            .expect("Failed to check key2 existence after delete");

        assert!(!exists1_after_delete);
        assert!(exists2_after_delete);

        client.del(key2).await.unwrap();
    }

    #[tokio::test]
    #[serial]
    async fn test_parallel_commands() {
        let mut config = test_config();
        config.request_timeout = Some(30000);

        let store = KeyValueStore::new(&config).await.unwrap();
        let mut client = store.client();

        let sleep_task = {
            let mut client = client.clone();
            tokio::spawn(async move {
                client
                    .send_command(redis::cmd("debug").arg("sleep").arg(2), None)
                    .await
                    .expect("Failed to sleep");
            })
        };

        // Give us enough time to be inside the sleep
        tokio::time::sleep(Duration::from_millis(50)).await;

        let timed_out = tokio::time::timeout(Duration::from_secs(5), async move {
            client.exists("").await.unwrap();
        })
        .await
        .is_err();

        assert!(!timed_out);
        sleep_task.await.unwrap();
    }

    #[tokio::test]
    #[serial]
    async fn test_connection_swapping() {
        let mut config = test_config();
        config.request_timeout = Some(30000);

        let store = KeyValueStore::new(&config).await.unwrap();
        let mut client1 = store.client();

        let sleep_task = tokio::spawn(async move {
            client1
                .send_command(redis::cmd("debug").arg("sleep").arg(3), None)
                .await
                .expect("Failed to sleep");
        });

        tokio::time::sleep(Duration::from_secs(2)).await;
        let mut client2 = store.client();

        client2
            .send_command(redis::cmd("debug").arg("sleep").arg(1), None)
            .await
            .expect("Failed to sleep");

        sleep_task.await.unwrap();
    }

    #[test]
    #[serial]
    fn test_deserialize_config_from_string() {
        let config_yaml = r#"
                host: "redis.example.com"
                port: 6379
                number_of_retries: 3
                auth:
                    fixed_token:
                        token: "test-token"
            "#;

        let config: KeyValueStoreConfig = serde_yaml::from_str(config_yaml).unwrap();

        assert_eq!(config.host, "redis.example.com");
        assert_eq!(config.port, 6379);
        assert_eq!(config.number_of_retries, 3);
        assert_eq!(config.connection_timeout, 5000);
        match config.auth.method {
            TokenSourceMethodConfig::FixedToken { token } => {
                assert_eq!(token, "test-token");
            }
            _ => panic!("Expected FixedToken auth method"),
        }
    }

    #[tokio::test]
    #[serial]
    async fn test_pipelining() {
        let config = test_config();

        let store = KeyValueStore::new(&config).await.unwrap();
        let mut client = store.client();

        let key = &random_key();

        let mut pipeline = redis::pipe();
        pipeline.get(key);
        pipeline.set_ex(key, 1, 60);
        pipeline.get(key);

        let (first, _, second): (Option<u32>, (), u32) = redis::from_redis_value(
            &client
                .send_pipeline(
                    &pipeline,
                    None,
                    true,
                    None,
                    PipelineRetryStrategy::new(false, false),
                )
                .await
                .unwrap(),
        )
        .unwrap();

        assert!(first.is_none());
        assert_eq!(1, second);
    }
}
