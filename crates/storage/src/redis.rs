use anyhow::{Context, Result};
use deadpool_redis::{Config, Pool, Runtime};
use dotenvy::dotenv;
use redis::{AsyncCommands, FromRedisValue, Script};
use std::fmt::Debug;
use tokio::sync::OnceCell;

static REDIS_SERVICE: OnceCell<RedisService> = OnceCell::const_new();

pub async fn service() -> &'static RedisService {
    REDIS_SERVICE
        .get_or_init(|| async {
            dotenv().ok();
            let redis_url = std::env::var("REDIS_URL").expect("DATABASE_URL missing");
            RedisService::new(redis_url.as_str()).expect("Failed to initialize RedisService")
        })
        .await
}
#[derive(Clone)]
pub struct RedisService {
    pool: Pool,
}

impl RedisService {
    /// 初始化 Redis 连接池
    pub fn new(redis_url: &str) -> Result<Self> {
        let cfg = Config::from_url(redis_url);
        // 使用 Tokio 运行时创建连接池
        let pool = cfg
            .create_pool(Some(Runtime::Tokio1))
            .context("Failed to create Redis pool")?;

        Ok(Self { pool })
    }

    async fn get_connection(&self) -> Result<deadpool_redis::Connection> {
        self.pool
            .get()
            .await
            .context("Failed to get redis connection")
    }

    /// 基础 Set 操作
    /// 基础 Set 操作
    pub async fn set(&self, key: &str, value: &str, expire_seconds: Option<u64>) -> Result<()> {
        let mut conn = self.get_connection().await?;

        let _: () = conn.set(key, value).await?;

        if let Some(seconds) = expire_seconds {
            let _: () = conn.expire(key, seconds as i64).await?;
        }
        Ok(())
    }
    /// 基础 Get 操作
    pub async fn get(&self, key: &str) -> Result<Option<String>> {
        let mut conn = self.get_connection().await?;
        let result: Option<String> = conn.get(key).await?;
        Ok(result)
    }

    pub async fn delete(&self, key: &str) -> Result<()> {
        let mut conn = self.get_connection().await?;
        let _: () = conn.del(key).await?;
        Ok(())
    }

    /// 【核心功能】执行 Lua 脚本 (原子性/事务性)
    ///
    /// # 参数
    /// * `script_content`: Lua 脚本代码
    /// * `keys`: 涉及到的 Key 列表 (Lua 中用 KEYS[1], KEYS[2]...)
    /// * `args`: 涉及到的参数列表 (Lua 中用 ARGV[1], ARGV[2]...)
    pub async fn exec_lua_script<T>(
        &self,
        script_content: &str,
        keys: &[&str],
        args: &[&str], // 这里简化为字符串，也可以泛型化
    ) -> Result<T>
    where
        T: FromRedisValue + Debug,
    {
        let mut conn = self.get_connection().await?;

        // 1. 创建 Script 对象
        let mut script = Script::new(script_content);

        // 2. 注入 Keys
        for key in keys {
            script.key(key);
        }

        // 3. 注入 Args
        for arg in args {
            script.arg(arg);
        }

        // 4. 原子执行
        let result: T = script
            .invoke_async(&mut conn)
            .await
            .context(format!("Failed to execute lua script: {}", script_content))?;

        Ok(result)
    }
}
