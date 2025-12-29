use crate::enums::Side;
use crate::enums::StrategyStatus; // 引入 StrategyStatus
use crate::primitive::{CurrencyPair, Price, Quantity};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::FromRow;
use std::str::FromStr;
use uuid::Uuid;

/// 策略实例实体 (Strategy Instance Entity)
///
/// 对应数据库表: `strategies`
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Strategy {
    /// 数据库物理主键 (自增 ID)
    #[sqlx(rename = "id")]
    pub id: i64,

    /// 策略业务唯一标识 (UUID)
    #[sqlx(rename = "uuid")]
    pub uuid: String,

    /// 策略实例名称
    pub name: String,

    /// 策略逻辑类名
    pub class_name: String,

    /// 策略当前生命周期状态
    pub status: StrategyStatus,

    /// 状态变更原因 / 错误信息 / 备注
    #[sqlx(default)]
    pub reason: String,

    /// 策略静态配置参数 (JSON)
    pub config: Value,

    /// 记录创建时间
    pub gmt_create: DateTime<Utc>,

    /// 记录最后修改时间
    pub gmt_modified: DateTime<Utc>,
}

/// 策略运行时状态快照 (Strategy Runtime State Snapshot)
///
/// 对应数据库表: `strategy_states`
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct StrategyState {
    #[sqlx(rename = "id")]
    pub id: i64,

    /// 关联的策略业务 UUID (Foreign Key)
    #[sqlx(rename = "strategy_uuid")]
    pub uuid: String,

    /// 策略运行时动态数据 (JSON)
    pub state_data: Value,

    pub gmt_create: DateTime<Utc>,
    pub gmt_modified: DateTime<Utc>,
}

/// 策略生成的交易信号 (Signal)
///
/// 对应数据库表: `signals`
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Signal {
    #[sqlx(rename = "id")]
    pub id: i64,

    /// 信号业务唯一标识 (UUID)
    #[sqlx(rename = "uuid")]
    pub uuid: String,

    /// 归属策略的业务 UUID
    #[sqlx(rename = "strategy_uuid")]
    pub strategy_uuid: String,

    /// 交易标的
    /// 类型: CurrencyPair (BASE/QUOTE)
    /// 数据库存储: VARCHAR ("BTC/USDT")
    pub symbol: CurrencyPair,

    /// 交易方向
    pub side: Side,

    /// 建议成交价格 (限价)
    pub price: Option<Price>,

    /// 建议成交数量
    pub quantity: Option<Quantity>,

    /// 信号触发原因
    pub reason: String,

    pub gmt_create: DateTime<Utc>,
    pub gmt_modified: DateTime<Utc>,
}

// =========================================================================
// 实现部分 (impl)
// =========================================================================

impl Strategy {
    /// 创建一个新的策略实例
    pub fn new(name: impl Into<String>, class_name: impl Into<String>, config: Value) -> Self {
        let now = Utc::now();
        Self {
            id: 0,
            uuid: Uuid::new_v4().to_string(),
            name: name.into(),
            class_name: class_name.into(),
            status: StrategyStatus::Created,
            reason: String::new(), // 默认为空
            config,
            gmt_create: now,
            gmt_modified: now,
        }
    }

    pub fn is_running(&self) -> bool {
        self.status == StrategyStatus::Running
    }

    pub fn is_active(&self) -> bool {
        matches!(
            self.status,
            StrategyStatus::Running | StrategyStatus::Initializing
        )
    }
}

impl Signal {
    /// 创建一个限价交易信号
    pub fn new_limit(
        strategy_uuid: String,
        symbol: impl Into<String>,
        side: Side,
        price: Price,
        quantity: Quantity,
        reason: impl Into<String>,
    ) -> Self {
        let now = Utc::now();
        // 解析 Symbol 字符串为 CurrencyPair
        // 注意：这里简单使用 unwrap，实际上游应保证格式正确
        let symbol_str: String = symbol.into();
        let pair = CurrencyPair::from_str(&symbol_str).expect("Invalid symbol format for Signal");

        Self {
            id: 0,
            uuid: Uuid::new_v4().to_string(),
            strategy_uuid,
            symbol: pair,
            side,
            price: Some(price),
            quantity: Some(quantity),
            reason: reason.into(),
            gmt_create: now,
            gmt_modified: now,
        }
    }

    /// 创建一个市价交易信号 (不带价格)
    pub fn new_market(
        strategy_uuid: String,
        symbol: impl Into<String>,
        side: Side,
        quantity: Quantity,
        reason: impl Into<String>,
    ) -> Self {
        let now = Utc::now();
        let symbol_str: String = symbol.into();
        let pair = CurrencyPair::from_str(&symbol_str).expect("Invalid symbol format for Signal");

        Self {
            id: 0,
            uuid: Uuid::new_v4().to_string(),
            strategy_uuid,
            symbol: pair,
            side,
            price: None,
            quantity: Some(quantity),
            reason: reason.into(),
            gmt_create: now,
            gmt_modified: now,
        }
    }
}

impl StrategyState {
    pub fn new(strategy_uuid: String, state_data: Value) -> Self {
        let now = Utc::now();
        Self {
            id: 0,
            uuid: strategy_uuid,
            state_data,
            gmt_create: now,
            gmt_modified: now,
        }
    }
}
