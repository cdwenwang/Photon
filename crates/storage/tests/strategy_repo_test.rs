#[cfg(test)]
mod tests {
    use quant_core::enums::{Side, StrategyStatus};
    use quant_core::primitive::{Price, Quantity};
    use quant_core::strategy::{Signal, Strategy};
    use quant_storage::repository::strategy_repo;

    use anyhow::Result;
    use rust_decimal_macros::dec;
    use serde_json::json;
    use std::str::FromStr;
    use uuid::Uuid;

    // =========================================================================
    // 辅助函数
    // =========================================================================
    async fn get_test_repo() -> strategy_repo::StrategyRepository {
        let pool = quant_storage::repository::common::get_real_pool().await;
        strategy_repo::StrategyRepository::new(pool.clone())
    }

    // 辅助函数：生成唯一名称，避免 Duplicate entry 错误
    fn unique_name(prefix: &str) -> String {
        format!("{}_{}", prefix, Uuid::new_v4().simple())
    }

    // =========================================================================
    // 1. 策略生命周期：注册、查询与状态流转
    // =========================================================================
    #[tokio::test]
    async fn test_strategy_lifecycle() -> Result<()> {
        let repo = get_test_repo().await;

        // 1. 准备数据 (使用 unique_name)
        let name = unique_name("Test_Grid_V1");
        let class_name = "SpotGridStrategy";
        let config = json!({
            "grid_num": 10,
            "range": [30000, 40000]
        });

        let strategy = Strategy::new(name.clone(), class_name, config.clone());

        // 2. 插入数据库
        let rows = repo.create(&strategy).await?;
        assert_eq!(rows, 1, "Should insert 1 strategy row");

        // 3. 查询验证
        let strategy_uuid = Uuid::from_str(&strategy.uuid)?;
        let found_opt = repo.find_by_uuid(strategy_uuid).await?;

        assert!(found_opt.is_some(), "Strategy should be found by UUID");
        let found = found_opt.unwrap();

        assert_eq!(found.uuid, strategy.uuid);
        assert_eq!(found.name, name); // 验证名称
        assert!(matches!(found.status, StrategyStatus::Created));

        // 4. 状态流转测试
        repo.update_status(strategy_uuid, StrategyStatus::Running)
            .await?;

        let updated = repo.find_by_uuid(strategy_uuid).await?.unwrap();
        assert!(matches!(updated.status, StrategyStatus::Running));

        Ok(())
    }

    // =========================================================================
    // 2. 列表查询：活跃策略筛选
    // =========================================================================
    #[tokio::test]
    async fn test_find_active_strategies() -> Result<()> {
        let repo = get_test_repo().await;

        // 1. 创建三个策略 (使用 unique_name 确保每次运行都不重复)
        let s1 = Strategy::new(unique_name("Active_Run"), "ClassA", json!({}));
        let s2 = Strategy::new(unique_name("Active_Init"), "ClassB", json!({}));
        let s3 = Strategy::new(unique_name("Inactive_Stop"), "ClassC", json!({}));

        repo.create(&s1).await?;
        repo.create(&s2).await?;
        repo.create(&s3).await?;

        // 2. 修改数据库中的状态
        let u1 = Uuid::from_str(&s1.uuid)?;
        let u2 = Uuid::from_str(&s2.uuid)?;
        let u3 = Uuid::from_str(&s3.uuid)?;

        repo.update_status(u1, StrategyStatus::Running).await?; // 活跃
        repo.update_status(u2, StrategyStatus::Initializing).await?; // 活跃
        repo.update_status(u3, StrategyStatus::Stopped).await?; // 非活跃

        // 3. 调用 find_active_strategies
        let active_list = repo.find_active_strategies().await?;

        // 4. 验证结果
        let active_uuids: Vec<String> = active_list.into_iter().map(|s| s.uuid).collect();

        assert!(
            active_uuids.contains(&s1.uuid),
            "Should contain Running strategy"
        );
        assert!(
            active_uuids.contains(&s2.uuid),
            "Should contain Initializing strategy"
        );
        // 注意：数据库里可能还有以前测试留下的活跃策略，所以不能用 len() 判断
        // 但一定不能包含本次创建的 s3
        assert!(
            !active_uuids.contains(&s3.uuid),
            "Should NOT contain Stopped strategy"
        );

        Ok(())
    }

    // =========================================================================
    // 3. 运行时状态：快照保存与加载 (Upsert)
    // =========================================================================
    #[tokio::test]
    async fn test_strategy_state_persistence() -> Result<()> {
        let repo = get_test_repo().await;
        // 模拟一个策略ID (无需真实存在于 strategy 表，除非有外键强约束)
        let strategy_uuid = Uuid::new_v4();

        // 1. 构造初始状态
        let initial_data = json!({
            "cash_balance": 10000.0,
            "positions": []
        });

        // 2. 保存
        repo.save_state(strategy_uuid, &initial_data).await?;

        // 验证
        let loaded = repo.load_state(strategy_uuid).await?.unwrap();
        assert_eq!(loaded.uuid, strategy_uuid.to_string());
        assert_eq!(loaded.state_data, initial_data);

        // 3. 更新 (测试 Duplicate Key Update)
        let updated_data = json!({
            "cash_balance": 5000.0,
            "positions": [{"symbol": "BTC/USDT", "amt": 0.1}]
        });

        repo.save_state(strategy_uuid, &updated_data).await?;

        let reloaded = repo.load_state(strategy_uuid).await?.unwrap();
        assert_eq!(reloaded.state_data, updated_data);

        Ok(())
    }

    // =========================================================================
    // 4. 信号记录：限价单与市价单
    // =========================================================================
    #[tokio::test]
    async fn test_signal_logging() -> Result<()> {
        let repo = get_test_repo().await;
        // 如果有外键约束，这里需要先创建一个真实的策略
        // 为了安全起见，我们先创建一个策略
        let strategy_name = unique_name("Signal_Test_Strat");
        let strategy = Strategy::new(strategy_name, "SignalTest", json!({}));
        repo.create(&strategy).await?;

        let strategy_uuid_str = strategy.uuid.clone();

        // 1. 构造信号
        let signal_limit = Signal::new_limit(
            strategy_uuid_str.clone(),
            "BTC/USDT",
            Side::Buy,
            Price(dec!(50000.0)),
            Quantity(dec!(0.5)),
            "RSI_Oversold",
        );

        let signal_market = Signal::new_market(
            strategy_uuid_str.clone(),
            "ETH/USDT",
            Side::Sell,
            Quantity(dec!(2.0)),
            "Stop_Loss",
        );

        // 2. 插入信号
        repo.save_signal(&signal_limit).await?;
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        repo.save_signal(&signal_market).await?;

        // 3. 查询验证
        let strategy_uuid = Uuid::from_str(&strategy_uuid_str)?;

        // 测试 Limit = 1 (返回最新)
        let latest = repo.find_signals_by_strategy(strategy_uuid, 1).await?;
        assert_eq!(latest.len(), 1);
        assert_eq!(latest[0].uuid, signal_market.uuid);
        assert_eq!(latest[0].symbol.to_string(), "ETH/USDT");

        // 测试全部
        let all = repo.find_signals_by_strategy(strategy_uuid, 10).await?;
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].uuid, signal_market.uuid);
        assert_eq!(all[1].uuid, signal_limit.uuid);

        Ok(())
    }
}
