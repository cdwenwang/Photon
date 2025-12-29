use sqlx::MySqlPool;
use tokio::sync::OnceCell;

// 1. 引入 Core 定义的实体和错误
use crate::repository::common;
use anyhow::Result;
use quant_core::account::{Asset, Position};

static ACCOUNT_POOL: OnceCell<AccountRepository> = OnceCell::const_new();
pub async fn repository() -> &'static AccountRepository {
    ACCOUNT_POOL
        .get_or_init(|| async {
            let pool = common::get_db_pool().await;
            AccountRepository::new(pool.clone())
        })
        .await
}

#[derive(Clone)]
pub struct AccountRepository {
    pool: MySqlPool,
}

impl AccountRepository {
    pub fn new(pool: MySqlPool) -> Self {
        Self { pool }
    }
    pub async fn upsert_asset(&self, asset: &Asset) -> Result<u64> {
        let result = sqlx::query!(
            r#"
        INSERT INTO asset (
            uuid, account_name, exchange, currency, 
            free, frozen, borrowed
        )
        VALUES (?, ?, ?, ?, ?, ?, ?)
        ON DUPLICATE KEY UPDATE 
            free = VALUES(free), 
            frozen = VALUES(frozen), 
            borrowed = VALUES(borrowed)
        "#,
            asset.uuid.to_string(),
            asset.account_name,
            asset.exchange.to_string(),
            asset.currency,
            asset.free,
            asset.frozen,
            asset.borrowed
        )
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected())
    }

    /// 查询某账户下的所有资产余额
    pub async fn find_assets_by_account(&self, account_name: &str) -> Result<Vec<Asset>> {
        let assets = sqlx::query_as::<_, Asset>(
            r#"
            SELECT 
                id, uuid, account_name, exchange, currency, 
                free, frozen, borrowed, 
                gmt_create, gmt_modified
            FROM asset
            WHERE account_name = ?
            "#,
        )
        .bind(account_name)
        .fetch_all(&self.pool)
        .await?; // 转换错误

        Ok(assets)
    }

    /// 查询特定币种的余额
    pub async fn find_asset(
        &self,
        account_name: &str,
        exchange: &str,
        currency: &str,
    ) -> Result<Option<Asset>> {
        let asset = sqlx::query_as::<_, Asset>(
            r#"
            SELECT 
                id, uuid, account_name, exchange, currency, 
                free, frozen, borrowed, 
                gmt_create, gmt_modified
            FROM asset
            WHERE account_name = ? AND exchange = ? AND currency = ?
            "#,
        )
        .bind(account_name)
        .bind(exchange)
        .bind(currency)
        .fetch_optional(&self.pool)
        .await?; // 转换错误

        Ok(asset)
    }

    // =========================================================================
    // 2. Position (持仓)
    // =========================================================================

    /// 同步持仓信息 (Upsert)
    pub async fn upsert_position(&self, pos: &Position) -> Result<u64> {
        let result = sqlx::query!(
            r#"
            INSERT INTO `position` (
                uuid, account_name, exchange, symbol, side, 
                quantity, entry_price, unrealized_pnl, leverage
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON DUPLICATE KEY UPDATE 
                quantity = VALUES(quantity),
                entry_price = VALUES(entry_price),
                unrealized_pnl = VALUES(unrealized_pnl),
                leverage = VALUES(leverage)
            "#,
            pos.uuid.to_string(),
            pos.account_name,
            pos.exchange,
            pos.symbol,
            pos.side.to_string(),
            pos.quantity,
            pos.entry_price,
            pos.unrealized_pnl,
            pos.leverage
        )
        .execute(&self.pool)
        .await?; // 转换错误

        Ok(result.rows_affected())
    }

    /// 查询某账户下的所有持仓
    pub async fn find_positions_by_account(&self, account_name: &str) -> Result<Vec<Position>> {
        let positions = sqlx::query_as::<_, Position>(
            r#"
            SELECT 
                id, uuid, account_name, exchange, symbol, side, 
                quantity, entry_price, unrealized_pnl, leverage,
                gmt_create, gmt_modified
            FROM `position`
            WHERE account_name = ?
            "#,
        )
        .bind(account_name)
        .fetch_all(&self.pool)
        .await?;

        Ok(positions)
    }

    /// 清空某账户的所有持仓
    pub async fn clear_positions(&self, account_name: &str) -> Result<()> {
        sqlx::query!(
            "DELETE FROM `position` WHERE account_name = ?",
            account_name
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }
}
