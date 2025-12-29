use sqlx::MySqlPool;

// 声明子模块
pub mod db;
pub mod models;
pub mod redis;
pub mod repository;

#[derive(Clone)]
pub struct Storage {}

impl Storage {
    pub fn new(pool: MySqlPool) -> Self {
        Self {}
    }
}
