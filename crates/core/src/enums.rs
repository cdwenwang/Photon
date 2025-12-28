// crates/core/src/enums.rs
use serde::{Deserialize, Serialize};
use sqlx::Type;
use strum::{Display, EnumString};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Display, EnumString, Type)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")] // 序列化为 "BUY", "SELL"
#[strum(serialize_all = "SCREAMING_SNAKE_CASE")]
#[sqlx(type_name = "VARCHAR")]
#[sqlx(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Side {
    Buy,
    Sell,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Display, EnumString, Type)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[sqlx(type_name = "VARCHAR")]
#[sqlx(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum OrderType {
    Limit,
    Market,
    StopLoss,
    Ioc, // Immediate or Cancel
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Display, EnumString, Default, Type,
)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[strum(serialize_all = "SCREAMING_SNAKE_CASE")]
#[sqlx(type_name = "VARCHAR")]
#[sqlx(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum OrderStatus {
    #[default]
    Created, // 本地已创建，未发送
    Pending,         // 已发送，等待交易所确认
    New,             // 交易所已确认挂单
    PartiallyFilled, // 部分成交
    Filled,          // 全部成交
    Canceled,        // 已撤单
    Rejected,        // 拒单
    Expired,         // 过期
}

// =========================================================================
// StrategyStatus (策略生命周期状态)
// =========================================================================

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    Default,
    Serialize,
    Deserialize,
    Display,
    EnumString,
    Type,
)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")] // JSON: "RUNNING"
#[strum(serialize_all = "SCREAMING_SNAKE_CASE")] // DB/String: "RUNNING"
#[sqlx(type_name = "VARCHAR")]
#[sqlx(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum StrategyStatus {
    #[default]
    Created, // 已创建，未初始化
    Initializing, // 初始化中 (加载历史数据、计算指标预热)
    Running,      // 运行中 (正常接收行情并交易)
    Paused,       // 已暂停 (仅接收行情，不触发新订单，通常用于人工干预)
    Stopping,     // 停止中 (正在执行收尾工作，如平掉所有持仓)
    Stopped,      // 已停止 (彻底结束，不再消耗资源)
    Error,        // 故障 (因异常而停止，需要人工检查日志)
}

impl StrategyStatus {
    /// 检查策略是否处于活跃状态 (可以进行逻辑处理)
    pub fn is_active(&self) -> bool {
        matches!(self, StrategyStatus::Running | StrategyStatus::Initializing)
    }

    /// 检查策略是否可以交易
    pub fn can_trade(&self) -> bool {
        matches!(self, StrategyStatus::Running)
    }

    /// 检查策略是否已终结
    pub fn is_finished(&self) -> bool {
        matches!(self, StrategyStatus::Stopped | StrategyStatus::Error)
    }
}

/// K 线周期
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize, Display, EnumString, sqlx::Type)]
#[sqlx(type_name = "VARCHAR")]
#[sqlx(rename_all = "SCREAMING_SNAKE_CASE")]
#[strum(serialize_all = "SCREAMING_SNAKE_CASE")]
pub enum BarPeriod {
    #[default]
    M1,  // 1分钟
    M5,  // 5分钟
    M15, // 15分钟
    H1,  // 1小时
    H4,  // 4小时
    D1,  // 日线
}