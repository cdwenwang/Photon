use crate::enums::{BarPeriod, Exchange}; // 引入 Exchange 枚举
use crate::primitive::{CurrencyPair, Price, Quantity};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

/// 市场 K 线实体 (Market Bar / Candlestick)
///
/// 对应数据库表: `market_bar`
///
/// 该结构体代表了特定时间周期内（如 1分钟、1小时）的市场价格聚合数据 (OHLCV)。
///
/// ### 架构设计优化:
/// 1. **强类型化**: `exchange` 和 `symbol` 不再是裸字符串，而是使用了 `Exchange` 枚举和 `CurrencyPair` 结构体。
///    这保证了业务逻辑中不会出现无效的交易所名称或格式错误的交易对。
/// 2. **数据库兼容**: 通过实现 `sqlx::Type`，`CurrencyPair` 会自动序列化为 `"BTC/USDT"` 字符串存入数据库，
///    读取时自动解析回结构体，做到了**业务对象化，存储扁平化**。
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct MarketBar {
    /// 数据库物理主键 (自增 ID)
    #[sqlx(rename = "id")]
    pub id: i64,

    /// 交易所
    /// 类型: Enum (Binance, Okx...)
    /// 数据库存储: VARCHAR ("BINANCE")
    pub exchange: Exchange,

    /// 交易标的
    /// 类型: Struct { base: "BTC", quote: "USDT" }
    /// 数据库存储: VARCHAR ("BTC/USDT")
    pub symbol: CurrencyPair,

    /// K 线周期
    /// 数据库存储: VARCHAR ("M1", "H1")
    #[sqlx(rename = "bar_period")]
    pub bar_period: BarPeriod,

    /// 开盘价 (Open)
    pub open: Price,

    /// 最高价 (High)
    pub high: Price,

    /// 最低价 (Low)
    pub low: Price,

    /// 收盘价 (Close)
    pub close: Price,

    /// 成交量 (Volume - Base Asset)
    pub volume: Quantity,

    /// 成交额 (Amount - Quote Asset)
    pub amount: Option<Decimal>,

    /// K 线开始时间 (ms)
    pub start_time: i64,

    /// K 线结束时间 (ms)
    pub end_time: i64,

    /// 入库时间
    pub gmt_create: DateTime<Utc>,
}

impl MarketBar {
    /// 创建一个新的 K 线实例
    ///
    /// 参数 `symbol` 支持传入 `CurrencyPair` 结构体，或者符合 "BASE/QUOTE" 格式的字符串。
    ///
    /// ### 示例:
    /// ```code
    /// // 方式 1: 使用强类型
    /// let pair = CurrencyPair::new("BTC", "USDT");
    /// MarketBar::new(Exchange::Binance, pair, ...);
    ///
    /// // 方式 2: 使用字符串 (会自动 parse，失败会 panic，建议仅在测试用)
    /// // 实际生产中建议在上游就转换好 CurrencyPair
    /// ```
    pub fn new(
        exchange: Exchange,
        symbol: impl Into<String>, // 这里为了方便，可以保留 Into<String> 然后内部 parse，或者直接要求 CurrencyPair
        bar_period: BarPeriod,
        open: Price,
        high: Price,
        low: Price,
        close: Price,
        volume: Quantity,
        start_time: i64,
    ) -> anyhow::Result<Self> {
        // 注意：这里返回值变成了 Result，因为字符串解析可能失败

        let duration_ms = Self::period_ms(bar_period);

        // 解析 Symbol
        let symbol_str: String = symbol.into();
        // 假设 instrument::CurrencyPair 实现了 FromStr
        use std::str::FromStr;
        let currency_pair = CurrencyPair::from_str(&symbol_str)?;

        Ok(Self {
            id: 0,
            exchange,
            symbol: currency_pair,
            bar_period,
            open,
            high,
            low,
            close,
            volume,
            amount: None,
            start_time,
            end_time: start_time + duration_ms,
            gmt_create: Utc::now(),
        })
    }

    pub fn new_with_pair(
        exchange: Exchange,
        base: &str,
        quote: &str,
        bar_period: BarPeriod,
        open: Price,
        high: Price,
        low: Price,
        close: Price,
        volume: Quantity,
        start_time: i64,
    ) -> Self {
        let duration_ms = Self::period_ms(bar_period);
        Self {
            id: 0,
            exchange,
            symbol: CurrencyPair::new(base, quote),
            bar_period,
            open,
            high,
            low,
            close,
            volume,
            amount: None,
            start_time,
            end_time: start_time + duration_ms,
            gmt_create: Utc::now(),
        }
    }

    fn period_ms(p: BarPeriod) -> i64 {
        match p {
            BarPeriod::M1 => 60 * 1000,
            BarPeriod::M5 => 5 * 60 * 1000,
            BarPeriod::M15 => 15 * 60 * 1000,
            BarPeriod::H1 => 3600 * 1000,
            BarPeriod::H4 => 4 * 3600 * 1000,
            BarPeriod::D1 => 24 * 3600 * 1000,
        }
    }
}
