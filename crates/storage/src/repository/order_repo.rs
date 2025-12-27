use anyhow::Result;
use quant_core::Order;
use serde::{Deserialize, Serialize};
use sqlx::MySqlPool;
#[derive(Debug, Clone)]
pub struct OrderRepository {
    pool: MySqlPool,
}

impl OrderRepository {
    pub fn new(pool: MySqlPool) -> Self {
        Self { pool }
    }

    // ❌ 之前是 insert(&self, order: &Quantity)
    // ✅ 改为 &Order，因为 Quantity 里面没有 symbol, id 这些字段
    pub async fn insert(&self, order: &Order) -> Result<u64> {
        let result = sqlx::query!(
            r#"
            INSERT INTO orders (id, symbol, side, price, quantity, status, created_at)
            VALUES (?, ?, ?, ?, ?, ?, ?)
            "#,
            order.id.to_string(),
            order.symbol,
            order.side.to_string(),
            // 这里要小心：如果 price 是 None，需要处理。这里简单演示用 unwrap
            // 注意：rust_decimal 存入 MySQL 需要对应 decimal 类型
            order.price.unwrap().0,
            order.quantity.0,
            order.status.to_string(),
            order.created_at
        )
            .execute(&self.pool)
            .await?;

        Ok(result.rows_affected())
    }
}