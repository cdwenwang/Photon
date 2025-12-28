use sqlx::MySqlPool;

// 声明子模块
pub mod db;
pub mod models;
pub mod repository;

// 引入 OrderRepository
use crate::repository::account_repo::AccountRepository;
use crate::repository::market_repo::MarketDataRepository;
use crate::repository::order_repo::OrderRepository;
use crate::repository::strategy_repo::StrategyRepository;

#[derive(Clone)]
pub struct Storage {
    pub order: OrderRepository,
    pub strategy: StrategyRepository,
    pub account: AccountRepository,
    pub market: MarketDataRepository,
}

impl Storage {
    pub fn new(pool: MySqlPool) -> Self {
        Self {
            order: OrderRepository::new(pool.clone()),
            strategy: StrategyRepository::new(pool.clone()),
            account: AccountRepository::new(pool.clone()),
            market: MarketDataRepository::new(pool.clone()),
        }
    }
}
