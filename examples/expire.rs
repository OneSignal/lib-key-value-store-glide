use std::{
    fs::File,
    sync::{
        atomic::{AtomicU32, Ordering},
        Arc,
    },
    time::Duration,
};

use key_value_store_glide::{
    KeyValueStore, KeyValueStoreConfig, KeyValueTokenSource, TokenSourceError,
    REFRESH_INTERVAL_SECS,
};

#[derive(Default, Clone)]
struct ExampleTokenSource {
    pub token_count: Arc<AtomicU32>,
}

#[async_trait::async_trait]
impl KeyValueTokenSource for ExampleTokenSource {
    async fn get_token(&self) -> Result<String, TokenSourceError> {
        self.token_count.fetch_add(1, Ordering::Relaxed);

        Ok("development_password".to_string())
    }
}

// This examples shows how to use a custom KeyValueTokenSource
// and that it will be called every REFRESH_INTERVAL_SECS
#[tokio::main]
pub async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config_path =
        std::env::var("TEST_CONFIG_PATH").unwrap_or_else(|_| "config-local.yml".to_string());
    let file = File::open(config_path)?;
    let config: KeyValueStoreConfig = serde_yaml::from_reader(file)?;

    let source = Box::new(ExampleTokenSource::default());

    let key_value_store = KeyValueStore::new_with_token_source(&config, source.clone()).await?;

    let _client = key_value_store.client();
    println!("Waiting for next refresh (1 minute)");

    tokio::time::sleep(Duration::from_secs(REFRESH_INTERVAL_SECS + 5)).await;
    let _client = key_value_store.client();
    assert_eq!(2, source.token_count.load(Ordering::Relaxed));

    Ok(())
}
