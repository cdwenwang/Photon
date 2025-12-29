#[cfg(test)]
mod tests {
    use quant_core::enums::{BarPeriod, Exchange};
    use quant_core::market::MarketBar;
    use quant_core::primitive::{Price, Quantity};
    // 假设 Price/Quantity 在这里
    use quant_storage::repository::market_repo;

    use rust_decimal_macros::dec;
    use uuid::Uuid;
    // =========================================================================
    // 1. 辅助函数 (Mock Data)
    // =========================================================================

    async fn get_test_repo() -> market_repo::MarketDataRepository {
        // 复用你的 get_db_pool，但它会返回一个新的 Pool 实例
        let pool = quant_storage::repository::common::get_real_pool().await;
        market_repo::MarketDataRepository::new(pool.clone())
    }

    /// 生成一个随机的 K 线数据
    fn mock_bar(
        exchange: Exchange,
        symbol_str: &str,
        period: BarPeriod,
        start_time: i64,
    ) -> MarketBar {
        // 构造价格和数量 (假设 Price 和 Quantity 是包含 Decimal 的 Tuple Struct)
        let open = Price(dec!(50000.0));
        let high = Price(dec!(51000.0));
        let low = Price(dec!(49000.0));
        let close = Price(dec!(50500.0));
        let volume = Quantity(dec!(1.5));

        MarketBar::new_with_pair(
            exchange,
            symbol_str.split('/').next().unwrap(), // Base: "BTC"
            symbol_str.split('/').nth(1).unwrap(), // Quote: "USDT"
            period,
            open,
            high,
            low,
            close,
            volume,
            start_time,
        )
    }

    // =========================================================================
    // 2. 测试用例
    // =========================================================================

    /// 测试保存和更新逻辑 (Upsert)
    #[tokio::test]
    async fn test_save_and_update_bar() -> Result<(), Box<dyn std::error::Error>> {
        let repo = get_test_repo().await;

        // 使用随机 Symbol 防止并发测试冲突
        let unique_symbol = format!("BTC_{}/USDT", &Uuid::new_v4().simple().to_string()[..8]);
        let period = BarPeriod::M1;
        let start_time = 1600000000000; // 固定时间戳

        // 1. 创建并保存初始 K 线
        let mut bar = mock_bar(Exchange::Binance, &unique_symbol, period, start_time);
        bar.close = Price(dec!(50500.0)); // 初始收盘价

        let rows = repo.save(&bar).await?;
        assert!(rows >= 1, "Should insert new bar");

        // 2. 查询验证插入结果
        let saved_bars = repo
            .find_recent_bars(
                "BINANCE", // 注意：find 参数是 &str，需与数据库存储一致
                &unique_symbol,
                period,
                1,
            )
            .await?;

        assert_eq!(saved_bars.len(), 1);
        assert_eq!(saved_bars[0].close.0, dec!(50500.0));

        // 3. 修改数据再次保存 (测试 ON DUPLICATE KEY UPDATE)
        bar.close = Price(dec!(60000.0)); // 修改收盘价
        bar.volume = Quantity(dec!(10.0)); // 修改成交量

        let update_rows = repo.save(&bar).await?;
        // MySQL upsert update 返回 2 (如果真的修改了数据) 或 1 (视驱动版本而定)，只要不报错即可
        assert!(update_rows >= 1, "Should update existing bar");

        // 4. 再次查询验证更新结果
        let updated_bars = repo
            .find_recent_bars("BINANCE", &unique_symbol, period, 1)
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
        assert_eq!(
            updated_bars[0].start_time, start_time,
            "Start time should maintain identity"
        );

        Ok(())
    }

    /// 测试查询最近 K 线 (Pagination & Sorting)
    #[tokio::test]
    async fn test_find_recent_bars() -> Result<(), Box<dyn std::error::Error>> {
        let repo = get_test_repo().await;
        let unique_symbol = format!("ETH_{}/USDT", &Uuid::new_v4().simple().to_string()[..8]);
        let period = BarPeriod::H1;

        let base_time = 1600000000000;

        // 1. 插入 3 根连续的 K 线 (T1, T2, T3)
        let bar1 = mock_bar(Exchange::Okx, &unique_symbol, period, base_time);
        let bar2 = mock_bar(Exchange::Okx, &unique_symbol, period, base_time + 3600_000);
        let bar3 = mock_bar(Exchange::Okx, &unique_symbol, period, base_time + 7200_000);

        repo.save(&bar1).await?;
        repo.save(&bar2).await?;
        repo.save(&bar3).await?;

        // 2. 查询最近的 2 根
        let recent_bars = repo
            .find_recent_bars(
                "OKX", // 对应 Exchange::Okx
                &unique_symbol,
                period,
                2, // Limit 2
            )
            .await?;

        // 3. 验证结果
        // SQL: ORDER BY start_time DESC，所以应该是 [T3, T2]
        assert_eq!(recent_bars.len(), 2);
        assert_eq!(
            recent_bars[0].start_time, bar3.start_time,
            "First bar should be the latest (T3)"
        );
        assert_eq!(
            recent_bars[1].start_time, bar2.start_time,
            "Second bar should be T2"
        );

        Ok(())
    }

    /// 测试按时间范围查询 (Time Range Filtering)
    #[tokio::test]
    async fn test_find_bars_by_range() -> Result<(), Box<dyn std::error::Error>> {
        let repo = get_test_repo().await;
        let unique_symbol = format!("SOL_{}/USDT", &Uuid::new_v4().simple().to_string()[..8]);
        let period = BarPeriod::D1;
        let day_ms = 86400_000;
        let t0 = 1000 * day_ms; // 假设起始时间

        // 1. 准备数据: T1, T2, T3, T4
        let b1 = mock_bar(Exchange::Binance, &unique_symbol, period, t0 + day_ms); // Day 1
        let b2 = mock_bar(Exchange::Binance, &unique_symbol, period, t0 + 2 * day_ms); // Day 2
        let b3 = mock_bar(Exchange::Binance, &unique_symbol, period, t0 + 3 * day_ms); // Day 3
        let b4 = mock_bar(Exchange::Binance, &unique_symbol, period, t0 + 4 * day_ms); // Day 4

        repo.save(&b1).await?;
        repo.save(&b2).await?;
        repo.save(&b3).await?;
        repo.save(&b4).await?;

        // 2. 查询范围: [Day 2, Day 3]
        // 包含 T2 和 T3，排除 T1 和 T4
        let start_range = t0 + 2 * day_ms;
        let end_range = t0 + 3 * day_ms;

        let range_bars = repo
            .find_bars_by_range("BINANCE", &unique_symbol, period, start_range, end_range)
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
            "First bar should be T2"
        );
        assert_eq!(
            range_bars[1].start_time, b3.start_time,
            "Second bar should be T3"
        );

        Ok(())
    }
}
