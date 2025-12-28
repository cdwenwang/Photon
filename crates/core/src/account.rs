use crate::enums::{Exchange, Side};
use crate::primitive::CurrencyPair; // 👈 必须引入 CurrencyPair
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use std::str::FromStr;
use uuid::Uuid;

// =========================================================================
// Asset (资产余额)
// =========================================================================

/// 资产余额实体 (Asset Balance)
///
/// 对应数据库表: `asset`
///
/// 该结构体记录了策略或账户在特定交易所的资金快照。
/// 这是一个“存量”概念，用于风控检查资金是否充足，以及计算总账户净值。
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Asset {
    /// 数据库物理主键 (自增 ID)
    #[sqlx(rename = "id")]
    pub id: i64,

    /// 资产业务唯一标识 (UUID)
    #[sqlx(rename = "uuid")]
    pub uuid: Uuid,

    /// 账户组/别名
    pub account_name: String,

    /// 交易所名称 (枚举)
    pub exchange: Exchange,

    /// 币种名称
    /// 示例: "USDT", "BTC"
    /// 注意: 这里通常是单个币种，不是交易对，所以保持 String
    pub currency: String,

    /// 可用余额
    pub free: Decimal,

    /// 冻结余额
    pub frozen: Decimal,

    /// 借贷/负债
    pub borrowed: Decimal,

    pub gmt_create: DateTime<Utc>,
    pub gmt_modified: DateTime<Utc>,
}

impl Asset {
    /// 创建一个新的资产记录实例
    pub fn new(account: &str, exchange: Exchange, currency: &str) -> Self {
        let now = Utc::now();
        Self {
            id: 0,
            uuid: Uuid::new_v4(),
            account_name: account.to_string(),
            // ✅ 直接赋值枚举
            exchange,
            currency: currency.to_string(),
            free: Decimal::ZERO,
            frozen: Decimal::ZERO,
            borrowed: Decimal::ZERO,
            gmt_create: now,
            gmt_modified: now,
        }
    }

    /// 计算总权益 (Total Equity)
    pub fn total(&self) -> Decimal {
        self.free + self.frozen - self.borrowed
    }
}

// =========================================================================
// Position (持仓)
// =========================================================================

/// 持仓实体 (Position)
///
/// 对应数据库表: `position`
///
/// 该结构体记录了当前的合约或现货持仓风险暴露。
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Position {
    /// 数据库物理主键 (自增 ID)
    #[sqlx(rename = "id")]
    pub id: i64,

    /// 持仓业务唯一标识 (UUID)
    #[sqlx(rename = "uuid")]
    pub uuid: Uuid,

    /// 账户组/别名
    pub account_name: String,

    /// 交易所 (枚举)
    pub exchange: Exchange,

    /// 交易标的 / 交易对
    /// ⚠️ 升级: String -> CurrencyPair (确保类型安全)
    /// 数据库存储: VARCHAR ("BTC/USDT")
    pub symbol: CurrencyPair,

    /// 持仓方向
    pub side: Side,

    /// 持仓数量 (绝对值)
    pub quantity: Decimal,

    /// 开仓均价
    pub entry_price: Option<Decimal>,

    /// 未实现盈亏
    pub unrealized_pnl: Option<Decimal>,

    /// 杠杆倍数
    pub leverage: Decimal,

    pub gmt_create: DateTime<Utc>,
    pub gmt_modified: DateTime<Utc>,
}

impl Position {
    /// 创建一个新的持仓记录实例
    ///
    /// `symbol` 参数支持传入字符串 (如 "BTC/USDT")，内部会自动解析为 `CurrencyPair`。
    pub fn new(account: &str, exchange: Exchange, symbol: impl Into<String>, side: Side) -> Self {
        let now = Utc::now();

        // 解析 Symbol
        let symbol_str: String = symbol.into();
        let pair = CurrencyPair::from_str(&symbol_str)
            .expect("Invalid symbol format for Position (expected BASE/QUOTE)");

        Self {
            id: 0,
            uuid: Uuid::new_v4(),
            account_name: account.to_string(),
            // ✅ 直接赋值枚举
            exchange,
            // ✅ 使用解析后的强类型
            symbol: pair,
            side,
            quantity: Decimal::ZERO,
            entry_price: None,
            unrealized_pnl: None,
            leverage: Decimal::ONE,
            gmt_create: now,
            gmt_modified: now,
        }
    }
}
