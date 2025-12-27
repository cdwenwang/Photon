use sqlx::mysql::{MySqlPool, MySqlPoolOptions};
use anyhow::{Context, Result};
use std::time::Duration;
use tracing::info;

/// 初始化数据库连接池
///
/// # 参数
/// * `database_url`: 数据库连接字符串 (e.g., mysql://root:123456@localhost:3306/photon_db)
pub async fn init_db(database_url: &str) -> Result<MySqlPool> {
    // 1. 配置连接池参数
    // 这些参数对于生产环境非常重要
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
        .connect(database_url)
        .await
        .context("Failed to connect to MySQL database. Please check your .env configuration and ensure MySQL is running.")?;

    // 3. (可选) 验证连接是否真正可用
    // 发送一个简单的 ping 确保网络通畅
    // 这一步能避免“连接池建好了，但其实连不上”的尴尬
    sqlx::query("SELECT 1")
        .execute(&pool)
        .await
        .context("Failed to execute ping query")?;

    info!("✅ Successfully connected to MySQL database pool.");

    // 4. (可选) 自动运行数据库迁移
    // 如果你在 migrations 文件夹里写了 SQL，取消下面的注释，启动时会自动建表
    // sqlx::migrate!("./migrations")
    //     .run(&pool)
    //     .await
    //     .context("Failed to run database migrations")?;

    Ok(pool)
}