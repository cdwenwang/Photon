use anyhow::{Context, Result};
use deadpool_redis::{Config, Pool, Runtime};
use dotenvy::dotenv;
use redis::{AsyncCommands, FromRedisValue, Script};
use std::fmt::Debug;
use std::time::Duration;
use tokio::sync::OnceCell;
use tokio::time::sleep;

static REDIS_SERVICE: OnceCell<RedisService> = OnceCell::const_new();

pub async fn service() -> &'static RedisService {
    REDIS_SERVICE
        .get_or_init(|| async {
            dotenv().ok();
            let redis_url = std::env::var("REDIS_URL").expect("REDIS_URL missing");
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
        args: &[&str],
    ) -> Result<T>
    where
        T: FromRedisValue + Debug,
    {
        let mut conn = self.get_connection().await?;

        // 构建 EVAL 命令
        // 格式: EVAL script numkeys key1 key2 ... arg1 arg2 ...
        let mut cmd = redis::cmd("EVAL");

        // 1. 脚本内容
        cmd.arg(script_content);

        // 2. Key 的数量 (Redis 协议要求必须传这个)
        cmd.arg(keys.len());

        // 3. 注入所有 Keys
        for key in keys {
            cmd.arg(*key);
        }

        // 4. 注入所有 Args
        for arg in args {
            cmd.arg(*arg);
        }

        // 5. 执行
        let result: T = cmd
            .query_async(&mut conn)
            .await
            .context(format!("Failed to execute lua script: {}", script_content))?;

        Ok(result)
    }

    // ========================================================================
    //  分布式锁 (Distributed Lock)
    // ========================================================================

    /// 尝试获取分布式锁 (非阻塞，一次性)
    ///
    /// # 参数
    /// * `key`: 锁的键名
    /// * `token`: 锁的持有者标识 (必须全局唯一，建议使用 Uuid::new_v4().to_string())
    /// * `ttl_ms`: 锁的过期时间 (毫秒)，防止死锁
    ///
    /// # 返回
    /// * `true`: 获取锁成功
    /// * `false`: 锁已被占用
    pub async fn try_lock(&self, key: &str, token: &str, ttl_ms: u64) -> Result<bool> {
        let script = r#"
            if redis.call("SET", KEYS[1], ARGV[1], "NX", "PX", ARGV[2]) then
                return 1
            else
                return 0
            end
        "#;

        // 【关键修改】显式转换并绑定变量，确保 args 切片中的引用有效
        let ttl_str = ttl_ms.to_string();

        // 构建参数切片
        let args = [token, ttl_str.as_str()];

        let result: i32 = self
            .exec_lua_script(
                script,
                &[key],
                &args, // 传入切片引用
            )
            .await?;

        Ok(result == 1)
    }

    /// 释放分布式锁 (原子操作)
    ///
    /// # 核心逻辑
    /// 只有当 Redis 中的 Value 等于传入的 token 时，才执行删除。
    /// 防止锁过期后，被其他线程获取，而当前线程误删了别人的锁。
    pub async fn unlock(&self, key: &str, token: &str) -> Result<bool> {
        let script = r#"
            if redis.call("GET", KEYS[1]) == ARGV[1] then
                return redis.call("DEL", KEYS[1])
            else
                return 0
            end
        "#;

        let result: i32 = self.exec_lua_script(script, &[key], &[token]).await?;

        // 1 表示删除成功，0 表示 Key 不存在或 Value 不匹配
        Ok(result == 1)
    }

    /// [高级封装] 自旋获取锁 (阻塞等待直到超时)
    ///
    /// 这是一个简单的自旋锁实现，适合需要等待锁的场景。
    ///
    /// # 参数
    /// * `wait_timeout_ms`: 最大等待获取锁的时间 (毫秒)
    /// * `retry_interval_ms`: 每次重试的间隔 (毫秒)
    pub async fn lock_with_retry(
        &self,
        key: &str,
        token: &str,
        ttl_ms: u64,
        wait_timeout_ms: u64,
        retry_interval_ms: u64,
    ) -> Result<bool> {
        let start = std::time::Instant::now();
        let timeout = Duration::from_millis(wait_timeout_ms);

        loop {
            // 1. 尝试加锁
            if self.try_lock(key, token, ttl_ms).await? {
                return Ok(true);
            }

            // 2. 检查是否超时
            if start.elapsed() >= timeout {
                return Ok(false);
            }

            // 3. 睡眠等待下一次重试
            sleep(Duration::from_millis(retry_interval_ms)).await;
        }
    }
}
