use crate::repository::common;
use anyhow::Result;
use chrono::NaiveDate;
use quant_core::market::MarketBar;
use quant_core::BarPeriod;
use sqlx::MySqlPool;
use tokio::sync::OnceCell;

static MARKET_DATA_POOL: OnceCell<MarketDataRepository> = OnceCell::const_new();

pub async fn repository() -> &'static MarketDataRepository {
    MARKET_DATA_POOL
        .get_or_init(|| async {
            let pool = common::get_db_pool().await;
            MarketDataRepository::new(pool.clone())
        })
        .await
}

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
    /// 唯一索引变更为: (exchange, symbol, bar_period, start_time, type)
    pub async fn save(&self, bar: &MarketBar) -> Result<u64> {
        // 注意: type 是 SQL 关键字，必须加反引号 `type`
        let result = sqlx::query!(
            r#"
            INSERT INTO `market_bar` (
                exchange, symbol, bar_period, `type`,
                open, high, low, close, volume, amount, 
                start_time, end_time
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON DUPLICATE KEY UPDATE 
                open = VALUES(open),
                high = VALUES(high),
                low = VALUES(low),
                close = VALUES(close),
                volume = VALUES(volume),
                amount = VALUES(amount),
                end_time = VALUES(end_time)
            "#,
            bar.exchange,               // 假设 Exchange 实现了 sqlx::Type
            bar.symbol,                 // 假设 CurrencyPair 实现了 sqlx::Type
            bar.bar_period.to_string(), // BarPeriod 转字符串
            bar.trade_type,             // u8
            bar.open.0,                 // Price -> Decimal
            bar.high.0,
            bar.low.0,
            bar.close.0,
            bar.volume.0,   // Quantity -> Decimal
            bar.amount,     // Option<Decimal>
            bar.start_time, // NaiveDate -> SQL DATE
            bar.end_time    // NaiveDate -> SQL DATE
        )
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected())
    }

    /// 查询最近的 N 天 K 线
    ///
    /// 参数 trade_type: 通常查询普通交易(21)
    pub async fn find_recent_bars(
        &self,
        exchange: &str,
        symbol: &str,
        bar_period: BarPeriod,
        trade_type: u8,
        limit: i64,
    ) -> Result<Vec<MarketBar>> {
        let bars = sqlx::query_as::<_, MarketBar>(
            r#"
            SELECT 
                id, exchange, symbol, bar_period, `type`,
                open, high, low, close, volume, amount, 
                start_time, end_time, gmt_create
            FROM market_bar
            WHERE exchange = ? 
              AND symbol = ? 
              AND bar_period = ?
              AND `type` = ?
            ORDER BY start_time DESC
            LIMIT ?
            "#,
        )
        .bind(exchange)
        .bind(symbol)
        .bind(bar_period.to_string())
        .bind(trade_type)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        Ok(bars)
    }

    /// 查询指定日期范围的 K 线
    pub async fn find_bars_by_range(
        &self,
        exchange: &str,
        symbol: &str,
        bar_period: BarPeriod,
        trade_type: u8,
        start_date: NaiveDate,
        end_date: NaiveDate,
    ) -> Result<Vec<MarketBar>> {
        let bars = sqlx::query_as::<_, MarketBar>(
            r#"
            SELECT 
                id, exchange, symbol, bar_period, `type`,
                open, high, low, close, volume, amount, 
                start_time, end_time, gmt_create
            FROM market_bar
            WHERE exchange = ? 
              AND symbol = ? 
              AND bar_period = ?
              AND `type` = ?
              AND start_time >= ? 
              AND start_time <= ?
            ORDER BY start_time ASC
            "#,
        )
        .bind(exchange)
        .bind(symbol)
        .bind(bar_period.to_string())
        .bind(trade_type)
        .bind(start_date)
        .bind(end_date)
        .fetch_all(&self.pool)
        .await?;

        Ok(bars)
    }
}
