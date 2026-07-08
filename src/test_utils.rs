use std::path::PathBuf;

use eyre::Result;
use serde::Deserialize;

use crate::{KeyValueStore, KeyValueStoreConfig};

#[derive(Debug, Default, Eq, PartialEq)]
pub struct CacheStatistics {
    pub hit: usize,
    pub miss: usize,
    pub invalidate: usize,
}

pub fn random_key() -> String {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    let key: String = (0..10)
        .map(|_| rng.sample(rand::distributions::Alphanumeric) as char)
        .collect();
    key
}

#[test]
fn deserialize_config() {
    let _: KeyValueStoreConfig = load_config(Some("config-local.yml".into())).unwrap();
}

pub fn load_config<'de, T: Deserialize<'de>>(path: Option<PathBuf>) -> Result<T> {
    let mut builder = config::Config::builder();

    // if we have a provided file path, then add it
    if let Some(path) = path {
        builder = builder.add_source(config::File::from(path));
    }

    // We want to support setting configuration values from the environment which
    // means we must always add the environment source. There happens to be a bug
    // in this library, which makes it impossible to use `_` as the separator. So
    // to work around that bug, we simply use `__`. This does, however, mean that
    // when providing environment variable configuration options, you must use
    // `__` when a struct is nested. For example, in order to configure Scylla
    // username, you must use the following environment variable:
    // SCYLLA__USERNAME="username"
    //
    // This isn't ideal, but with my testing, this is still the best library
    // available to solve this problem.
    // Read more about the bug here:
    // https://github.com/mehcode/config-rs/issues/391
    builder = builder.add_source(config::Environment::default().separator("__"));

    Ok(builder.build()?.try_deserialize()?)
}

pub fn test_config() -> KeyValueStoreConfig {
    let config_path =
        std::env::var("TEST_CONFIG_PATH").unwrap_or_else(|_| "config-local.yml".to_string());

    load_config(Some(config_path.into())).unwrap()
}

pub async fn key_value_store() -> KeyValueStore {
    KeyValueStore::new(&test_config()).await.unwrap()
}
