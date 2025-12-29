use crate::repository::common;
use anyhow::Result;
use quant_core::enums::OrderStatus;
use quant_core::oms::Order;
use sqlx::MySqlPool;
use tokio::sync::OnceCell;
use uuid::Uuid;

static ORDER_POOL: OnceCell<OrderRepository> = OnceCell::const_new();

/// **获取订单数据仓储层实例**
pub async fn repository() -> &'static OrderRepository {
    ORDER_POOL
        .get_or_init(|| async {
            let pool = common::get_db_pool().await;
            OrderRepository::new(pool.clone())
        })
        .await
}

// ⚠️ 修复：添加 Clone
#[derive(Clone)]
pub struct OrderRepository {
    pool: MySqlPool,
}

impl OrderRepository {
    pub fn new(pool: MySqlPool) -> Self {
        Self { pool }
    }

    // insert 方法保持不变，因为它用的是 query! 宏来检查 SQL 语法，这很好
    // 只要传入参数类型匹配即可 (String 对 String, Decimal 对 Decimal)
    pub async fn insert(&self, order: &Order) -> Result<u64> {
        let result = sqlx::query!(
            r#"
            INSERT INTO `order` (
                order_uuid, strategy_uuid, exchange_order_id, 
                symbol, exchange, side, order_type, status, 
                price, quantity, filled_quantity, average_price, fee
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
            order.uuid.to_string(),
            order.strategy_uuid,
            order.exchange_order_id,
            order.symbol,
            order.exchange,
            order.side.to_string(),
            order.order_type.to_string(),
            order.status.to_string(),
            order.price.map(|p| p.0),
            order.quantity.0,
            order.filled_quantity.0,
            order.average_price.map(|p| p.0),
            order.fee
        )
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected())
    }

    /// ⚠️ 修复：改用 sqlx::query_as (函数版)
    pub async fn find_by_uuid(&self, order_uuid: Uuid) -> Result<Option<Order>> {
        let order = sqlx::query_as::<_, Order>(
            r#"
            SELECT 
                id, order_uuid, strategy_uuid, exchange_order_id,
                symbol, exchange, side, order_type, status,
                price, quantity, filled_quantity, average_price, fee,
                gmt_create, gmt_modified
            FROM `order`
            WHERE order_uuid = ?
            "#,
        )
        .bind(order_uuid.to_string()) // 手动绑定参数
        .fetch_optional(&self.pool)
        .await?;

        Ok(order)
    }

    /// ⚠️ 修复：改用 sqlx::query_as (函数版)
    pub async fn find_by_strategy(&self, strategy_uuid: Uuid) -> Result<Vec<Order>> {
        let orders = sqlx::query_as::<_, Order>(
            r#"
            SELECT 
                id, order_uuid, strategy_uuid, exchange_order_id,
                symbol, exchange, side, order_type, status,
                price, quantity, filled_quantity, average_price, fee,
                gmt_create, gmt_modified
            FROM `order`
            WHERE strategy_uuid = ?
            ORDER BY gmt_create DESC
            "#,
        )
        .bind(strategy_uuid.to_string())
        .fetch_all(&self.pool)
        .await?;

        Ok(orders)
    }

    // update_status 保持不变
    pub async fn update_status(
        &self,
        order_uuid: Uuid,
        status: OrderStatus,
        exchange_order_id: Option<String>,
        filled_qty: rust_decimal::Decimal,
        avg_price: Option<rust_decimal::Decimal>,
        fee: Option<rust_decimal::Decimal>,
    ) -> Result<()> {
        sqlx::query!(
            r#"
            UPDATE `order`
            SET status = ?, 
                exchange_order_id = COALESCE(?, exchange_order_id), 
                filled_quantity = ?, 
                average_price = ?,
                fee = ?
            WHERE order_uuid = ?
            "#,
            status.to_string(),
            exchange_order_id,
            filled_qty,
            avg_price,
            fee,
            order_uuid.to_string()
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }
}
