#[cfg(test)]
mod tests {
    // 假设你的 Core 实体位于这里，根据实际情况调整引用路径
    use quant_core::account::{Asset, Position};
    use quant_core::enums::{Exchange, Side};
    use quant_core::primitive::CurrencyPair;

    use quant_storage::repository::account_repo;
    use rust_decimal_macros::dec;
    use std::str::FromStr;
    use uuid::Uuid;
    // =========================================================================
    // 1. Mock 数据生成辅助函数
    // =========================================================================

    async fn get_test_repo() -> account_repo::AccountRepository {
        // 复用你的 get_db_pool，但它会返回一个新的 Pool 实例
        let pool = quant_storage::repository::common::get_real_pool().await;
        account_repo::AccountRepository::new(pool.clone())
    }

    fn mock_asset(account_name: &str) -> Asset {
        Asset {
            id: 0, // 插入时数据库自增，这里填0即可
            // 这里保留 Uuid 类型，因为你的 Repo 代码里做了 .to_string()
            uuid: Uuid::new_v4().to_string(),
            account_name: account_name.to_string(),
            // 假设 Exchange 是枚举
            exchange: Exchange::Binance,
            currency: "USDT".to_string(),
            free: dec!(1000.0),
            frozen: dec!(0.0),
            borrowed: dec!(0.0),
            gmt_create: chrono::Utc::now(),
            gmt_modified: chrono::Utc::now(),
        }
    }

    fn mock_position(account_name: &str) -> Position {
        Position {
            id: 0,
            uuid: Uuid::new_v4().to_string(),
            account_name: account_name.to_string(),
            exchange: Exchange::Okx,
            // 假设 Symbol 是 CurrencyPair 结构体
            symbol: CurrencyPair::from_str("ETH/USDT").unwrap(),
            side: Side::Buy,
            quantity: dec!(10.5),
            entry_price: Some(dec!(3000.0)),
            unrealized_pnl: Some(dec!(50.0)),
            leverage: dec!(10.0),
            gmt_create: chrono::Utc::now(),
            gmt_modified: chrono::Utc::now(),
        }
    }

    // =========================================================================
    // 2. Asset 测试 (测试字符串转换和 Upsert 逻辑)
    // =========================================================================

    #[tokio::test]
    async fn test_asset_flow() -> Result<(), Box<dyn std::error::Error>> {
        let repo = get_test_repo().await;
        // 使用随机后缀，防止测试并发冲突
        let account_name = format!("test_acc_{}", Uuid::new_v4());

        // 1. 创建并插入
        let mut asset = mock_asset(&account_name);
        // 修改一些特定值以便验证
        asset.free = dec!(500.50);

        // 调用 upsert_asset (内部会将 uuid 和 exchange 转为 string)
        let rows = repo.upsert_asset(&asset).await?;
        assert!(rows >= 1, "Upsert should affect at least 1 row");

        // 2. 查询验证
        // find_asset 参数是 &str，所以我们需要传入对应的字符串形式
        let exchange_str = "BINANCE"; // 对应 Exchange::Binance.to_string()
        let currency_str = "USDT";

        let found_opt = repo
            .find_asset(&account_name, exchange_str, currency_str)
            .await?;
        assert!(found_opt.is_some(), "Should find the inserted asset");

        let found = found_opt.unwrap();
        assert_eq!(found.free, dec!(500.50));
        assert_eq!(found.account_name, account_name);

        // 验证 UUID 是否正确回读 (SQLx 应该能把 VARCHAR(36) 读回 Uuid 类型)
        assert_eq!(found.uuid, asset.uuid);

        // 3. 更新测试 (Upsert Update)
        asset.free = dec!(999.99);
        repo.upsert_asset(&asset).await?;

        // 再次查询
        let updated_opt = repo
            .find_asset(&account_name, exchange_str, currency_str)
            .await?;
        let updated = updated_opt.unwrap();
        assert_eq!(updated.free, dec!(999.99), "Free balance should be updated");

        Ok(())
    }

    // =========================================================================
    // 3. Position 测试 (测试复杂类型字段)
    // =========================================================================

    #[tokio::test]
    async fn test_position_flow() -> Result<(), Box<dyn std::error::Error>> {
        let repo = get_test_repo().await;
        let account_name = format!("test_pos_{}", Uuid::new_v4());

        // 1. 创建持仓
        let mut pos = mock_position(&account_name);
        pos.quantity = dec!(2.0);
        pos.entry_price = Some(dec!(100.0));

        // 2. 插入
        // 注意：你的代码中 upsert_position 里：
        // pos.exchange 直接传了 (没调 to_string) -> 需要确保 Exchange 实现了 sqlx::Type 或者你修改代码加 .to_string()
        // pos.symbol  直接传了 -> 需要确保 CurrencyPair 实现了 sqlx::Type
        // pos.side.to_string() -> 也没问题
        repo.upsert_position(&pos).await?;

        // 3. 列表查询
        let list = repo.find_positions_by_account(&account_name).await?;
        assert_eq!(list.len(), 1);
        let saved_pos = &list[0];

        assert_eq!(saved_pos.quantity, dec!(2.0));
        // 验证枚举回读
        assert!(matches!(saved_pos.exchange, Exchange::Okx));
        assert!(matches!(saved_pos.side, Side::Buy));

        // 4. 清理 (Clear)
        repo.clear_positions(&account_name).await?;

        let empty_list = repo.find_positions_by_account(&account_name).await?;
        assert!(empty_list.is_empty(), "Positions should be cleared");

        Ok(())
    }
}
