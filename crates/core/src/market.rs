use crate::enums::{BarPeriod, Exchange};
use crate::primitive::{CurrencyPair, Price, Quantity};
use chrono::{DateTime, NaiveDate, Utc}; // 引入 NaiveDate 处理数据库的 DATE 类型
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use std::str::FromStr;

/// 市场 K 线实体 (Market Bar / Candlestick)
///
/// 对应数据库表: `market_bar` (针对日线及以上周期优化)
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct MarketBar {
    /// 数据库物理主键 (自增 ID)
    #[sqlx(rename = "id")]
    pub id: i64,

    /// 交易所
    /// 假设实现了 sqlx::Type<MySql>，如果未实现，需要在 Repository层 bind 时转为 String
    pub exchange: Exchange,

    /// 交易标的
    /// 假设实现了 sqlx::Type<MySql>，对应 VARCHAR
    pub symbol: CurrencyPair,

    /// K 线周期 (只支持 D1, W1 等日线级别，因为数据库只存 Date)
    #[sqlx(rename = "bar_period")]
    pub bar_period: BarPeriod,

    /// 交易条件类型 (对应 SQL 的 `type` 字段)
    /// 默认 21 (盘中交易)
    #[sqlx(rename = "type")]
    pub trade_type: u8,

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

    /// K 线开始日期 (DATE)
    pub start_time: NaiveDate,

    /// K 线结束日期 (DATE)
    pub end_time: NaiveDate,

    /// 入库时间
    pub gmt_create: DateTime<Utc>,
}

impl MarketBar {
    /// 创建一个新的 K 线实例
    pub fn new(
        exchange: Exchange,
        symbol: impl Into<String>,
        bar_period: BarPeriod,
        trade_type: u8, // 新增参数
        open: Price,
        high: Price,
        low: Price,
        close: Price,
        volume: Quantity,
        date: NaiveDate, // 参数改为日期
    ) -> anyhow::Result<Self> {
        // 解析 Symbol
        let symbol_str: String = symbol.into();
        let currency_pair = CurrencyPair::from_str(&symbol_str)?;

        // 对于日线数据，通常 start_time 和 end_time 是同一天
        // 如果有特殊需求（如周线），end_time 可能不同，这里简化处理为同一天
        Ok(Self {
            id: 0,
            exchange,
            symbol: currency_pair,
            bar_period,
            trade_type,
            open,
            high,
            low,
            close,
            volume,
            amount: None,
            start_time: date,
            end_time: date,
            gmt_create: Utc::now(),
        })
    }
}
