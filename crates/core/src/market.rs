use crate::enums::BarPeriod;
use crate::primitive::{Price, Quantity};
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
/// ### 架构设计注意:
/// 1. **存储优化**: 与 `Order` 或 `Strategy` 不同，K 线数据量极大（单交易所单币种每年产生数十万条数据）。
///    因此，**不使用 UUID** 作为主键，而是使用 `(exchange, symbol, period, start_time)` 作为业务唯一键，
///    以减少索引占用的磁盘空间，提高写入和范围查询（Range Query）的性能。
/// 2. **数据流向**: 通常由 `Feed` 模块从交易所 WebSocket 接收，或者由 `DataRecorder` 模块定期合成，
///    最终存储于数据库供策略回测或实盘初始化计算使用。
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct MarketBar {
    /// 数据库物理主键 (自增 ID)
    /// 类型: BIGINT (i64)
    /// 作用: 仅用于数据库内部管理。业务查询通常依赖 `start_time` 范围索引。
    #[sqlx(rename = "id")]
    pub id: i64,

    /// 交易所名称 (e.g., "BINANCE")
    pub exchange: String,

    /// 交易标的 (e.g., "BTC/USDT")
    pub symbol: String,

    /// K 线周期 (Timeframe)
    /// 枚举: M1, M5, H1, D1 等
    /// #[sqlx(rename = "bar_period")]
    pub period: BarPeriod,

    /// 开盘价 (Open Price)
    pub open: Price,

    /// 最高价 (High Price)
    pub high: Price,

    /// 最低价 (Low Price)
    pub low: Price,

    /// 收盘价 (Close Price)
    pub close: Price,

    /// 成交量 (Base Asset Volume)
    /// 含义: 基础货币的交易数量 (如 BTC 数量)。
    pub volume: Quantity,

    /// 成交额 (Quote Asset Volume / Turnover)
    /// 含义: 计价货币的交易总额 (如 USDT 总额)。
    /// 注意: 部分交易所或数据源可能不提供此字段，故为 Option。
    pub amount: Option<Decimal>,

    /// K 线开始时间戳
    /// 格式: Unix Timestamp (毫秒 ms)
    /// 作用: K 线的唯一时间标识。
    pub start_time: i64,

    /// K 线结束时间戳
    /// 格式: Unix Timestamp (毫秒 ms)
    /// 计算方式: start_time + period_duration
    /// 作用: 用于判断 K 线是否已闭合 (Closed)。
    pub end_time: i64,

    /// 记录入库时间
    /// 作用: 记录数据被写入数据库的物理时间，用于延迟分析。
    pub gmt_create: DateTime<Utc>,
}

impl MarketBar {
    /// 创建一个新的 K 线实例
    ///
    /// ### 功能特性:
    /// 1. **自动计算结束时间**: 根据传入的 `period` 和 `start_time`，自动计算并填充 `end_time`。
    /// 2. **初始化默认值**: `id` 默认为 0，`amount` 默认为 None (后续可手动设置)。
    ///
    /// ### 参数:
    /// * `start_time`: K 线的起始时间戳 (毫秒)。
    pub fn new(
        exchange: impl Into<String>,
        symbol: impl Into<String>,
        period: BarPeriod,
        open: Price,
        high: Price,
        low: Price,
        close: Price,
        volume: Quantity,
        start_time: i64,
    ) -> Self {
        // 预先计算结束时间，避免外部手动计算出错
        let duration_ms = Self::period_ms(period);

        Self {
            id: 0,
            exchange: exchange.into(),
            symbol: symbol.into(),
            period,
            open,
            high,
            low,
            close,
            volume,
            amount: None, // 默认为空，如有需要需单独赋值
            start_time,
            end_time: start_time + duration_ms,
            gmt_create: Utc::now(),
        }
    }

    /// [Internal] 获取 K 线周期对应的毫秒数
    ///
    /// 用于计算 `end_time`。
    /// M1 = 60000ms, H1 = 3600000ms, etc.
    fn period_ms(p: BarPeriod) -> i64 {
        match p {
            BarPeriod::M1 => 60 * 1000,
            BarPeriod::M5 => 5 * 60 * 1000,
            BarPeriod::M15 => 15 * 60 * 1000, // 补充 M15
            BarPeriod::H1 => 3600 * 1000,
            BarPeriod::H4 => 4 * 3600 * 1000,
            BarPeriod::D1 => 24 * 3600 * 1000,
            _ => 0, // 对于未定义或不支持的周期，暂不增加时长
        }
    }
}
