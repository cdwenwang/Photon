use dotenvy::dotenv;
use sqlx::MySqlPool;
use tokio::sync::OnceCell;

static DB_POOL: OnceCell<MySqlPool> = OnceCell::const_new();
pub async fn get_db_pool() -> &'static MySqlPool {
    DB_POOL
        .get_or_init(|| async {
            dotenv().ok();
            let url = std::env::var("DATABASE_URL").expect("DATABASE_URL missing");
            MySqlPool::connect(&url).await.expect("DB connect failed")
        })
        .await
}
