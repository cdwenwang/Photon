use crate::enums::Side;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

/// 资产余额实体 (Asset Balance)
///
/// 对应数据库表: `asset`
///
/// 该结构体记录了策略或账户在特定交易所的资金快照。
/// 这是一个“存量”概念，用于风控检查资金是否充足，以及计算总账户净值。
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Asset {
    /// 数据库物理主键 (自增 ID)
    /// 类型: BIGINT (i64)
    /// 作用: 仅用于数据库内部索引，业务逻辑请使用 uuid。
    #[sqlx(rename = "id")]
    pub id: i64,

    /// 资产业务唯一标识 (UUID)
    /// 类型: VARCHAR(36)
    /// 作用: 全局唯一的记录 ID。
    #[sqlx(rename = "uuid")]
    pub uuid: Uuid,

    /// 账户组/别名
    /// 示例: "Main_Account", "Sub_Strategy_A"
    /// 作用: 用于区分同一交易所下的不同子账户或逻辑账户。
    pub account_name: String,

    /// 交易所名称
    /// 示例: "BINANCE", "OKX"
    pub exchange: String,

    /// 币种名称
    /// 示例: "USDT", "BTC", "ETH"
    /// 注意: 统一使用大写。
    pub currency: String,

    /// 可用余额 (Free/Available)
    /// 含义: 当前可以直接用于下单的金额。
    pub free: Decimal,

    /// 冻结余额 (Frozen/Locked)
    /// 含义: 已经挂单但尚未成交，或者被质押锁定的金额。
    /// 计算: 总持有量 = free + frozen。
    pub frozen: Decimal,

    /// 借贷/负债 (Borrowed)
    /// 含义: 杠杆交易中借入的金额。
    /// 作用: 计算净资产时需要减去此数值。
    pub borrowed: Decimal,

    /// 记录创建时间 (MySQL: gmt_create)
    pub gmt_create: DateTime<Utc>,

    /// 记录最后更新时间 (MySQL: gmt_modified)
    /// 作用: 判断资产数据的新鲜度，如果时间过久可能需要重新从 API 拉取。
    pub gmt_modified: DateTime<Utc>,
}

impl Asset {
    /// 创建一个新的资产记录实例
    pub fn new(account: &str, exchange: &str, currency: &str) -> Self {
        let now = Utc::now();
        Self {
            id: 0,
            uuid: Uuid::new_v4(),
            account_name: account.to_string(),
            exchange: exchange.to_string(),
            currency: currency.to_string(),
            free: Decimal::ZERO,
            frozen: Decimal::ZERO,
            borrowed: Decimal::ZERO,
            gmt_create: now,
            gmt_modified: now,
        }
    }

    /// 计算总权益 (Total Equity)
    ///
    /// 公式: 可用 + 冻结 - 借贷
    pub fn total(&self) -> Decimal {
        self.free + self.frozen - self.borrowed
    }
}

/// 持仓实体 (Position)
///
/// 对应数据库表: `position`
///
/// 该结构体记录了当前的合约或现货持仓风险暴露。
/// 这是一个“状态”概念，记录了你当前手里拿了多少货，成本是多少，浮盈浮亏是多少。
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

    /// 交易所名称
    pub exchange: String,

    /// 交易标的 / 交易对
    /// 示例: "BTC/USDT" (现货) 或 "BTC-USDT-SWAP" (永续合约)
    pub symbol: String,

    /// 持仓方向
    /// 枚举: Long (多头) / Short (空头)
    /// 注意: 现货通常只有 Long，合约可以有 Short。
    pub side: Side,

    /// 持仓数量 (绝对值)
    /// 含义: 当前持有的合约张数或币的数量。
    /// 注意: 通常存储为正数，通过 `side` 区分方向。
    pub quantity: Decimal,

    /// 开仓均价 (Average Entry Price)
    /// 含义: 持仓的加权平均成本价。
    /// 作用: 用于计算未实现盈亏。如果为 None，表示尚未建仓。
    pub entry_price: Option<Decimal>,

    /// 未实现盈亏 (Unrealized PnL)
    /// 含义: (当前标记价格 - 开仓均价) * 数量 * 方向系数。
    /// 作用: 动态反映当前持仓的浮动盈亏情况，风控核心指标。
    pub unrealized_pnl: Option<Decimal>,

    /// 杠杆倍数
    /// 默认: 1.0 (现货或1倍杠杆)
    pub leverage: Decimal,

    /// 记录创建时间
    pub gmt_create: DateTime<Utc>,

    /// 记录最后更新时间
    pub gmt_modified: DateTime<Utc>,
}

impl Position {
    /// 创建一个新的持仓记录实例
    pub fn new(account: &str, exchange: &str, symbol: &str, side: Side) -> Self {
        let now = Utc::now();
        Self {
            id: 0,
            uuid: Uuid::new_v4(),
            account_name: account.to_string(),
            exchange: exchange.to_string(),
            symbol: symbol.to_string(),
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
