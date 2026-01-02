#[cfg(test)]
mod tests {
    use anyhow::Result;
    use quant_core::enums::{BarPeriod, Exchange};
    use quant_core::market::MarketBar;
    use quant_core::primitive::{Price, Quantity};
    use quant_storage::repository::market_repo;

    use chrono::{Duration, NaiveDate};
    use rust_decimal_macros::dec;
    use uuid::Uuid;

    // =========================================================================
    // 1. 辅助函数 (Mock Data)
    // =========================================================================

    async fn get_test_repo() -> market_repo::MarketDataRepository {
        let pool = quant_storage::repository::common::get_real_pool().await;
        market_repo::MarketDataRepository::new(pool.clone())
    }

    /// 生成一个随机的 K 线数据 (适配新结构)
    fn mock_bar(
        exchange: Exchange,
        symbol_str: &str,
        period: BarPeriod,
        date: NaiveDate, // 参数改为日期
    ) -> MarketBar {
        let open = Price(dec!(50000.0));
        let high = Price(dec!(51000.0));
        let low = Price(dec!(49000.0));
        let close = Price(dec!(50500.0));
        let volume = Quantity(dec!(1.5));
        let trade_type = 21; // 默认盘中交易

        // 使用 new 方法，而不是 new_with_pair (假设你移除了旧的 new_with_pair)
        MarketBar::new(
            exchange, symbol_str, // 内部会自动 parse 为 CurrencyPair
            period, trade_type, open, high, low, close, volume, date,
        )
        .expect("Failed to create mock bar")
    }

    // =========================================================================
    // 2. 测试用例
    // =========================================================================

    /// 测试保存和更新逻辑 (Upsert)
    #[tokio::test]
    async fn test_save_and_update_bar() -> Result<()> {
        let repo = get_test_repo().await;

        let unique_symbol = format!("BTC_{}/USDT", &Uuid::new_v4().simple().to_string()[..8]);
        let period = BarPeriod::D1; // 日线
        let date = NaiveDate::from_ymd_opt(2023, 1, 1).unwrap();

        // 1. 创建并保存初始 K 线
        let mut bar = mock_bar(Exchange::Binance, &unique_symbol, period, date);
        bar.close = Price(dec!(50500.0));

        let rows = repo.save(&bar).await?;
        assert!(rows >= 1, "Should insert new bar");

        // 2. 查询验证插入结果
        let saved_bars = repo
            .find_recent_bars(
                "BINANCE",
                &unique_symbol,
                period,
                21, // trade_type
                1,
            )
            .await?;

        assert_eq!(saved_bars.len(), 1);
        assert_eq!(saved_bars[0].close.0, dec!(50500.0));
        assert_eq!(saved_bars[0].start_time, date);

        // 3. 修改数据再次保存 (测试 ON DUPLICATE KEY UPDATE)
        bar.close = Price(dec!(60000.0));
        bar.volume = Quantity(dec!(10.0));

        let update_rows = repo.save(&bar).await?;
        assert!(update_rows >= 1, "Should update existing bar");

        // 4. 再次查询验证更新结果
        let updated_bars = repo
            .find_recent_bars("BINANCE", &unique_symbol, period, 21, 1)
            .await?;

        assert_eq!(updated_bars.len(), 1);
        assert_eq!(
            updated_bars[0].close.0,
            dec!(60000.0),
            "Close price should be updated"
        );
        assert_eq!(
            updated_bars[0].volume.0,
            dec!(10.0),
            "Volume should be updated"
        );
        // 验证日期身份
        assert_eq!(updated_bars[0].start_time, date, "Start date should match");

        Ok(())
    }

    /// 测试查询最近 K 线 (Pagination & Sorting)
    #[tokio::test]
    async fn test_find_recent_bars() -> Result<()> {
        let repo = get_test_repo().await;
        let unique_symbol = format!("ETH_{}/USDT", &Uuid::new_v4().simple().to_string()[..8]);
        let period = BarPeriod::D1;

        let base_date = NaiveDate::from_ymd_opt(2023, 1, 1).unwrap();

        // 1. 插入 3 天连续的 K 线 (D1, D2, D3)
        let bar1 = mock_bar(Exchange::Okx, &unique_symbol, period, base_date);
        let bar2 = mock_bar(
            Exchange::Okx,
            &unique_symbol,
            period,
            base_date + Duration::days(1),
        );
        let bar3 = mock_bar(
            Exchange::Okx,
            &unique_symbol,
            period,
            base_date + Duration::days(2),
        );

        repo.save(&bar1).await?;
        repo.save(&bar2).await?;
        repo.save(&bar3).await?;

        // 2. 查询最近的 2 根
        let recent_bars = repo
            .find_recent_bars(
                "OKX",
                &unique_symbol,
                period,
                21, // type
                2,  // limit
            )
            .await?;

        // 3. 验证结果
        // SQL: ORDER BY start_time DESC，所以应该是 [D3, D2]
        assert_eq!(recent_bars.len(), 2);
        assert_eq!(
            recent_bars[0].start_time, bar3.start_time,
            "First bar should be the latest (D3)"
        );
        assert_eq!(
            recent_bars[1].start_time, bar2.start_time,
            "Second bar should be D2"
        );

        Ok(())
    }

    /// 测试按时间范围查询 (Time Range Filtering)
    #[tokio::test]
    async fn test_find_bars_by_range() -> Result<()> {
        let repo = get_test_repo().await;
        let unique_symbol = format!("SOL_{}/USDT", &Uuid::new_v4().simple().to_string()[..8]);
        let period = BarPeriod::D1;

        let t0 = NaiveDate::from_ymd_opt(2023, 6, 1).unwrap();

        // 1. 准备数据: D1, D2, D3, D4
        let b1 = mock_bar(Exchange::Binance, &unique_symbol, period, t0);
        let b2 = mock_bar(
            Exchange::Binance,
            &unique_symbol,
            period,
            t0 + Duration::days(1),
        );
        let b3 = mock_bar(
            Exchange::Binance,
            &unique_symbol,
            period,
            t0 + Duration::days(2),
        );
        let b4 = mock_bar(
            Exchange::Binance,
            &unique_symbol,
            period,
            t0 + Duration::days(3),
        );

        repo.save(&b1).await?;
        repo.save(&b2).await?;
        repo.save(&b3).await?;
        repo.save(&b4).await?;

        // 2. 查询范围: [D2, D3]
        // 包含 b2 和 b3，排除 b1 和 b4
        let start_range = t0 + Duration::days(1);
        let end_range = t0 + Duration::days(2);

        let range_bars = repo
            .find_bars_by_range(
                "BINANCE",
                &unique_symbol,
                period,
                21, // type
                start_range,
                end_range,
            )
            .await?;

        // 3. 验证
        // SQL: ORDER BY start_time ASC
        assert_eq!(
            range_bars.len(),
            2,
            "Should find exactly 2 bars inside range"
        );
        assert_eq!(
            range_bars[0].start_time, b2.start_time,
            "First bar should be D2"
        );
        assert_eq!(
            range_bars[1].start_time, b3.start_time,
            "Second bar should be D3"
        );

        Ok(())
    }
}
