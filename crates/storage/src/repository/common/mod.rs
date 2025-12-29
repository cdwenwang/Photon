use anyhow::Context;
use dotenvy::dotenv;
use sqlx::mysql::MySqlPoolOptions;
use sqlx::MySqlPool;
use std::time::Duration;
use tokio::sync::OnceCell;
use tracing::info;

static DB_POOL: OnceCell<MySqlPool> = OnceCell::const_new();
pub async fn get_db_pool() -> &'static MySqlPool {
    DB_POOL
        .get_or_init(|| async {
            dotenv().ok();
            let url = std::env::var("DATABASE_URL").expect("DATABASE_URL missing");
            let pool = MySqlPoolOptions::new()
                // 最大连接数：根据你的并发量设置。
                // 量化系统通常并发不高但响应要求快，20-50 足够。
                // 如果太多，MySQL 服务端可能会拒绝连接 (Too many connections)。
                .max_connections(20)

                // 最小空闲连接数：
                // 保持一定数量的连接“热着”，避免有新请求时还要花时间握手建立连接。
                .min_connections(5)

                // 获取连接超时：
                // 如果 3 秒拿不到连接，说明数据库挂了或者太忙，直接报错，不要死等。
                .acquire_timeout(Duration::from_secs(3))

                // 空闲超时：
                // 如果一个连接 10 分钟没人用，就把它关掉省资源。
                .idle_timeout(Duration::from_secs(600))

                // 2. 建立连接
                .connect(url.as_str())
                .await
                .context("Failed to connect to MySQL database. Please check your .env configuration and ensure MySQL is running.")
                .expect("Failed to connect to MySQL database.");

            // 3. 验证连接是否真正可用
            // 发送一个简单的 ping 确保网络通畅
            sqlx::query("SELECT 1")
                .execute(&pool)
                .await
                .context("Failed to execute ping query").expect("Ping query failed");

            info!("✅ Successfully connected to MySQL database pool.");
            return pool;
        })
        .await
}
