use crate::repository::common;
use anyhow::Result;
use quant_core::enums::StrategyStatus;
use quant_core::strategy::{Signal, Strategy, StrategyState};
use serde_json::Value;
use sqlx::MySqlPool;
use tokio::sync::OnceCell;
use uuid::Uuid;

static ORDER_POOL: OnceCell<StrategyRepository> = OnceCell::const_new();

/// **获取策略数据仓储层实例**
pub async fn repository() -> &'static StrategyRepository {
    ORDER_POOL
        .get_or_init(|| async {
            let pool = common::get_db_pool().await;
            StrategyRepository::new(pool.clone())
        })
        .await
}

/// 策略仓储层
/// 负责策略元数据、运行时状态和信号的持久化
#[derive(Clone)]
pub struct StrategyRepository {
    pool: MySqlPool,
}

impl StrategyRepository {
    pub fn new(pool: MySqlPool) -> Self {
        Self { pool }
    }

    // =========================================================================
    // 1. Strategy (策略元数据)
    // =========================================================================

    /// 创建/注册一个新策略
    pub async fn create(&self, strategy: &Strategy) -> Result<u64> {
        let result = sqlx::query!(
            r#"
            INSERT INTO `strategy` (uuid, name, class_name, status, config)
            VALUES (?, ?, ?, ?, ?)
            "#,
            strategy.uuid.to_string(),
            strategy.name,
            strategy.class_name,
            strategy.status.to_string(), // Enum -> String
            strategy.config              // SQLx 自动处理 serde_json::Value
        )
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected())
    }

    /// 根据 UUID 查询策略
    pub async fn find_by_uuid(&self, uuid: Uuid) -> Result<Option<Strategy>> {
        // 使用 query_as 函数而非宏，利用 FromRow  trait 进行自动映射
        // 这样可以自动处理 Enum 和 UUID 的转换
        let strategy = sqlx::query_as::<_, Strategy>(
            r#"
            SELECT
                id, uuid, name, class_name, status, config,
                gmt_create, gmt_modified
            FROM `strategy`
            WHERE uuid = ?
            "#,
        )
        .bind(uuid.to_string())
        .fetch_optional(&self.pool)
        .await?;

        Ok(strategy)
    }

    /// 获取所有需要启动的策略 (状态为 Running 或 Initializing)
    /// 系统启动时调用此方法加载策略
    pub async fn find_active_strategies(&self) -> Result<Vec<Strategy>> {
        let strategies = sqlx::query_as::<_, Strategy>(
            r#"
            SELECT
                id, uuid, name, class_name, status, config,
                gmt_create, gmt_modified
            FROM `strategy`
            WHERE status IN ('RUNNING', 'INITIALIZING')
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(strategies)
    }

    /// 更新策略状态
    pub async fn update_status(&self, uuid: Uuid, status: StrategyStatus) -> Result<()> {
        // 如果提供了 reason，则更新 reason；否则保持原值 (COALESCE)
        sqlx::query!(
            r#"
            UPDATE `strategy`
            SET status = ?
            WHERE uuid = ?
            "#,
            status.to_string(),
            uuid.to_string()
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    // =========================================================================
    // 2. StrategyState (运行时状态)
    // =========================================================================

    /// 保存/更新策略状态 (Upsert)
    /// 如果不存在则插入，如果存在则更新 state_data
    pub async fn save_state(&self, strategy_uuid: Uuid, state_data: &Value) -> Result<()> {
        // 注意：数据库表字段是 strategy_uuid，实体中对应的字段是 uuid
        sqlx::query!(
            r#"
            INSERT INTO `strategy_state` (strategy_uuid, state_data)
            VALUES (?, ?)
            ON DUPLICATE KEY UPDATE state_data = ?
            "#,
            strategy_uuid.to_string(),
            state_data,
            state_data
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// 加载策略状态
    pub async fn load_state(&self, strategy_uuid: Uuid) -> Result<Option<StrategyState>> {
        let state = sqlx::query_as::<_, StrategyState>(
            r#"
            SELECT
                id, strategy_uuid, state_data,
                gmt_create, gmt_modified
            FROM `strategy_state`
            WHERE strategy_uuid = ?
            "#,
        )
        .bind(strategy_uuid.to_string())
        .fetch_optional(&self.pool)
        .await?;

        Ok(state)
    }

    // =========================================================================
    // 3. Signal (交易信号)
    // =========================================================================

    /// 记录交易信号
    pub async fn save_signal(&self, signal: &Signal) -> Result<u64> {
        let result = sqlx::query!(
            r#"
            INSERT INTO `signal` (
                uuid, strategy_uuid, symbol, side,
                price, quantity, reason
            )
            VALUES (?, ?, ?, ?, ?, ?, ?)
            "#,
            signal.uuid.to_string(),
            signal.strategy_uuid.to_string(),
            signal.symbol,
            signal.side.to_string(),
            signal.price.map(|p| p.0), // Option<Price> -> Option<Decimal>
            signal.quantity.map(|q| q.0), // Option<Quantity> -> Option<Decimal>
            signal.reason
        )
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected())
    }

    /// 查询某策略最近发出的信号 (用于调试或UI展示)
    pub async fn find_signals_by_strategy(
        &self,
        strategy_uuid: Uuid,
        limit: i64,
    ) -> Result<Vec<Signal>> {
        let signals = sqlx::query_as::<_, Signal>(
            r#"
            SELECT
                id, uuid, strategy_uuid, symbol, side,
                price, quantity, reason,
                gmt_create, gmt_modified
            FROM `signal`
            WHERE strategy_uuid = ?
            ORDER BY gmt_create DESC
            LIMIT ?
            "#,
        )
        .bind(strategy_uuid.to_string())
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        Ok(signals)
    }
}
