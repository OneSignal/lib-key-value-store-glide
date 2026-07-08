
# key-value-store-glide

This crates provides a valkey-glide client which authenticates against GCP (Google Cloud Platform)
to obtain and refresh the required credential as needed.

## Version restrictions

The glide-core crate from valkey-glide lays a number of versioning restrictions we must abide by
to use it. These include:

- This must be included in your `.cargo/config.toml` file or your project will not build:
```
[env]
GLIDE_VERSION = "0.1.0"
```
- The _exact_ redis-rs included in this crate must be used if you want to interact with any redis-rs type
(calling `from_redis_value` for example).

There are built in helper methods in redis_extensions.rs to reduce the need for this, as this can be problematic
for some projects which include a more recent version of the same crate.

This crate reexports all of the vendored Redis, so when necessary it can be accessed even in projects with disjoint
redis-rs versions already included:

```
use key_value_store_glide::redis as glide_redis;
```

otherwise you may get errors like:

```
     |                        ---------------- ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ expected `redis::Value`, found `redis::types::Value`
     |                        |
     |                        arguments to this function are incorrect
     |
note: there are multiple different versions of crate `redis` in the dependency graph
```

See examples/pipeline.rs for details.

## KeyValueStore

The entrypoint of this library is `KeyValueStore` which is a wrapper around a `glide_core::client::Client`
from valkey-glide. This can be cloned and used in parallel.
However it is also cluster aware and allows pipelining and other cluster aware actions to be taken easily.

That said, the typical use case is initializing the struct, holding it, and clone it whenever you need to
execute a query against your memorystore.

Apart from this, the KeyValueStore will automatically handle reconnections and retries for us,
so we don't need to worry about that either.

### Configuration

`KeyValueStore::new` admits a `KeyValueStoreConfig`, which implements `serde::Deserialize`.

An example would be:

```
key_value_store:
  host: valkey
  port: 6379
  auth:
    fixed_token:
      token: development_password
```

You would typically use it inside another config struct of your own, like:

``` rust
#[derive(Deserialize, Clone)]
struct MyAwesomeServiceConfig {
  some_property: String,
  ...
  key_value_store: key_value_store::KeyValueStoreConfig,
  ...
}
```

Apart from this, You can also initialize programmatically:

```rust
let key_value_store_config = KeyValueStoreConfig{
  host: String::from("valkey"),
  port: 6379,
  auth: TokenSourceConfig {
    refresh_interval: 3600,
    method: TokenSourceMethodConfig::FixedToken("development_password")}
  }
};

```

Do note that `cluster_mode_enabled` should be set correctly in the configuration,
based upon if your are or are not connecting against a cluster-aware valkey setup (even if a single node).

## Examples

See the multiple examples in the examples directory for usage.
