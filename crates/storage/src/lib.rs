use sqlx::MySqlPool;

// 声明子模块
pub mod db;
pub mod models;
pub mod repository; // 这里会去找 repository/mod.rs

// 引入 OrderRepository
use repository::order_repo::OrderRepository;

#[derive(Clone)]
pub struct Storage {
    pub order: OrderRepository,
}

impl Storage {
    pub fn new(pool: MySqlPool) -> Self {
        Self {
            order: OrderRepository::new(pool.clone()),
        }
    }
}