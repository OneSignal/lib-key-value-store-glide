use std::fs::File;

use key_value_store_glide::{KeyValueStore, KeyValueStoreConfig};

// Pipelining requires using Redis type directly
// so use the re-exported version explicitly
use key_value_store_glide::redis as glide_redis;

#[tokio::main]
pub async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config_path =
        std::env::var("TEST_CONFIG_PATH").unwrap_or_else(|_| "config-local.yml".to_string());
    let file = File::open(config_path)?;
    let config: KeyValueStoreConfig = serde_yaml::from_reader(file)?;

    let store = KeyValueStore::new(&config).await.unwrap();
    let mut client = store.client();

    let key = "pipeline-key";

    let mut pipeline = glide_redis::pipe();
    pipeline.del(key);
    pipeline.get(key);
    pipeline.set_ex(key, 1, 60);
    pipeline.get(key);

    let (_, first, _, second): ((), Option<u32>, (), u32) = glide_redis::from_redis_value(
        &client
            .send_pipeline(
                &pipeline,
                None,
                true,
                None,
                glide_redis::PipelineRetryStrategy::new(false, false),
            )
            .await
            .unwrap(),
    )
    .unwrap();

    assert!(first.is_none());
    assert_eq!(1, second);

    Ok(())
}
