use redis::{from_redis_value, Cmd, FromRedisValue, RedisResult, ToRedisArgs};

#[async_trait::async_trait]
pub trait RedisExtensions {
    async fn del<'a, K: ToRedisArgs + Send>(&'a mut self, key: K) -> RedisResult<()>;

    async fn exists<'a, K: ToRedisArgs + Send>(&'a mut self, key: K) -> RedisResult<bool>;

    async fn set_ex<'a, K: ToRedisArgs + Send, V: ToRedisArgs + Send>(
        &'a mut self,
        key: K,
        value: V,
        seconds: u64,
    ) -> RedisResult<()>;

    async fn get<'a, K: ToRedisArgs + Send, V: FromRedisValue + Send>(
        &'a mut self,
        key: K,
    ) -> RedisResult<V>;
}

#[async_trait::async_trait]
impl RedisExtensions for glide_core::client::Client {
    async fn del<'a, K: ToRedisArgs + Send>(&'a mut self, key: K) -> RedisResult<()> {
        self.send_command(&mut Cmd::del(key), None).await?;
        Ok(())
    }

    async fn exists<'a, K: ToRedisArgs + Send>(&'a mut self, key: K) -> RedisResult<bool> {
        let result = self.send_command(&mut Cmd::exists(key), None).await?;
        from_redis_value(&result)
    }

    async fn set_ex<'a, K: ToRedisArgs + Send, V: ToRedisArgs + Send>(
        &'a mut self,
        key: K,
        value: V,
        seconds: u64,
    ) -> RedisResult<()> {
        self.send_command(&mut Cmd::set_ex(key, value, seconds), None)
            .await?;
        Ok(())
    }

    async fn get<'a, K: ToRedisArgs + Send, V: FromRedisValue + Send>(
        &'a mut self,
        key: K,
    ) -> RedisResult<V> {
        let result = self.send_command(&mut Cmd::get(key), None).await?;
        from_redis_value(&result)
    }
}
