use quant_core::enums::BarPeriod;
use quant_core::error::QuantError;
use quant_core::market::MarketBar;
use sqlx::MySqlPool;

/// 市场数据仓储层 (Market Data Repository)
///
/// 负责 K 线 (OHLCV) 数据的持久化和查询。
/// 针对高频写入和范围查询进行了优化。
#[derive(Clone)]
pub struct MarketDataRepository {
    pool: MySqlPool,
}

impl MarketDataRepository {
    pub fn new(pool: MySqlPool) -> Self {
        Self { pool }
    }

    /// 保存 K 线数据 (Upsert)
    ///
    /// 逻辑: 根据 (exchange, symbol, period, start_time) 唯一索引:
    /// - 不存在: 插入新记录。
    /// - 已存在: 更新 OHLCV 和成交量 (覆盖更新)。
    pub async fn save(&self, bar: &MarketBar) -> Result<u64, QuantError> {
        let result = sqlx::query!(
            r#"
            INSERT INTO `market_bar` (
                exchange, symbol, bar_period, 
                open, high, low, close, volume, amount, 
                start_time, end_time
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON DUPLICATE KEY UPDATE 
                open = VALUES(open),
                high = VALUES(high),
                low = VALUES(low),
                close = VALUES(close),
                volume = VALUES(volume),
                amount = VALUES(amount),
                end_time = VALUES(end_time)
            "#,
            bar.exchange,
            bar.symbol,
            bar.period.to_string(), // Enum -> String
            bar.open.0,             // Price -> Decimal
            bar.high.0,
            bar.low.0,
            bar.close.0,
            bar.volume.0, // Quantity -> Decimal
            bar.amount,   // Option<Decimal> (自动处理 None)
            bar.start_time,
            bar.end_time
        )
        .execute(&self.pool)
        .await
        .map_err(|e| QuantError::StorageError(e.to_string()))?;

        Ok(result.rows_affected())
    }

    /// 查询最近的 N 根 K 线 (用于策略初始化/计算指标)
    ///
    /// 排序: 按 start_time 倒序 (DESC)，取出最近的数据。
    /// 注意: 返回结果通常需要再反转回正序 (ASC) 给策略计算使用。
    pub async fn find_recent_bars(
        &self,
        exchange: &str,
        symbol: &str,
        period: BarPeriod,
        limit: i64,
    ) -> Result<Vec<MarketBar>, QuantError> {
        // 使用 query_as 函数版进行映射
        let bars = sqlx::query_as::<_, MarketBar>(
            r#"
            SELECT 
                id, exchange, symbol, period as "period: String", 
                open, high, low, close, volume, amount, 
                start_time, end_time, gmt_create
            FROM market_bar
            WHERE exchange = ? AND symbol = ? AND period = ?
            ORDER BY start_time DESC
            LIMIT ?
            "#,
        )
        .bind(exchange)
        .bind(symbol)
        .bind(period.to_string())
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| QuantError::StorageError(e.to_string()))?;

        Ok(bars)
    }

    /// 查询指定时间范围的 K 线 (用于回测)
    ///
    /// 排序: 按 start_time 正序 (ASC)
    pub async fn find_bars_by_range(
        &self,
        exchange: &str,
        symbol: &str,
        period: BarPeriod,
        start_ts: i64,
        end_ts: i64,
    ) -> Result<Vec<MarketBar>, QuantError> {
        let bars = sqlx::query_as::<_, MarketBar>(
            r#"
            SELECT 
                id, exchange, symbol, period as "period: String", 
                open, high, low, close, volume, amount, 
                start_time, end_time, gmt_create
            FROM market_bar
            WHERE exchange = ? 
              AND symbol = ? 
              AND period = ?
              AND start_time >= ? 
              AND start_time <= ?
            ORDER BY start_time ASC
            "#,
        )
        .bind(exchange)
        .bind(symbol)
        .bind(period.to_string())
        .bind(start_ts)
        .bind(end_ts)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| QuantError::StorageError(e.to_string()))?;

        Ok(bars)
    }
}
