use std::fs::File;

use key_value_store_glide::{KeyValueStore, KeyValueStoreConfig, RedisExtensions};

use glide_redis::Cmd;
use key_value_store_glide::redis as glide_redis;

// This example shows how to call Valkey APIs both built in and custom
#[tokio::main]
pub async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config_path =
        std::env::var("TEST_CONFIG_PATH").unwrap_or_else(|_| "config-local.yml".to_string());
    let file = File::open(config_path)?;
    let config: KeyValueStoreConfig = serde_yaml::from_reader(file)?;

    let key_value_store = KeyValueStore::new(&config).await?;

    // fetches a connection. This will internally create a new glide client
    // on top of the current one.
    let mut client = key_value_store.client();

    client.set_ex("key1", "value1", 60).await?;
    let value: String = client.get("key1").await?;

    assert_eq!(value, String::from("value1"));

    assert!(client.exists("key1").await?);
    client.del("key1").await?;

    assert!(!client.exists("key1").await?);

    // In addition to the built in convenience commands
    // you can invoke any Redis command using send_command
    // from the re-exported Redis
    let original_value: Option<u32> = client.get("key2").await?;

    client.send_command(&mut Cmd::incr("key2", 1), None).await?;

    let value: u32 = client.get("key2").await?;
    assert_eq!(original_value.unwrap_or_default() + 1, value);

    Ok(())
}
