use std::str::FromStr;

#[cfg(test)]
mod tests {
    use quant_core::enums::{Exchange, OrderStatus, Side};
    use quant_core::oms::Order;
    use quant_core::primitive::{Price, Quantity};

    use anyhow::Result;
    use quant_storage::repository::order_repo;
    use rust_decimal_macros::dec;
    use std::str::FromStr;
    use uuid::Uuid;

    async fn get_test_repo() -> order_repo::OrderRepository {
        // 复用你的 get_db_pool，但它会返回一个新的 Pool 实例
        let pool = quant_storage::repository::common::get_real_pool().await;
        order_repo::OrderRepository::new(pool.clone())
    }

    // =========================================================================
    // 1. 基础流程：插入与查询
    // =========================================================================
    #[tokio::test]
    async fn test_order_lifecycle() -> Result<()> {
        let repo = get_test_repo().await;

        // 1. 准备数据
        let strategy_id = Uuid::new_v4().to_string();
        let symbol = "BTC/USDT";
        let price = Price(dec!(50000.0));
        let quantity = Quantity(dec!(0.5));

        // 创建一个限价单 (内存中)
        let order = Order::new_limit(
            symbol,
            Exchange::Binance,
            Some(strategy_id.clone()),
            Side::Buy,
            price.clone(),
            quantity.clone(),
        );

        // 2. 插入数据库
        let rows = repo.insert(&order).await?;
        assert_eq!(rows, 1, "Should insert 1 row");

        // 3. 查询验证
        // 注意：repo 方法接受 Uuid 类型，但实体里存的是 String，需要解析
        let search_uuid = Uuid::from_str(&order.uuid)?;
        let found_opt = repo.find_by_uuid(search_uuid).await?;

        assert!(found_opt.is_some(), "Order should be found");
        let found = found_opt.unwrap();

        // 4. 深度断言
        assert_eq!(found.uuid, order.uuid);
        assert_eq!(found.strategy_uuid, Some(strategy_id));
        assert_eq!(found.symbol.to_string(), "BTC/USDT"); // 验证 CurrencyPair 序列化
        assert!(matches!(found.exchange, Exchange::Binance)); // 验证枚举
        assert!(matches!(found.status, OrderStatus::Created));
        assert_eq!(found.price.unwrap().0, dec!(50000.0));
        assert_eq!(found.quantity.0, dec!(0.5));
        assert_eq!(found.filled_quantity.0, dec!(0.0)); // 初始成交量应为 0

        Ok(())
    }

    // =========================================================================
    // 2. 更新流程：成交与状态变更
    // =========================================================================
    #[tokio::test]
    async fn test_order_status_update() -> Result<()> {
        let repo = get_test_repo().await;

        // 1. 创建并插入初始订单
        let order = Order::new_market(
            "ETH/USDT",
            Exchange::Okx,
            None, // 手动下单，无策略ID
            Side::Sell,
            Quantity(dec!(10.0)),
        );
        repo.insert(&order).await?;
        let order_uuid = Uuid::from_str(&order.uuid)?;

        // 2. 模拟交易所回报：部分成交 (PartiallyFilled)
        // 假设成交了 5 个，均价 3000，产生了交易所ID
        let exchange_oid = "ex_ord_123456".to_string();
        repo.update_status(
            order_uuid,
            OrderStatus::PartiallyFilled,
            Some(exchange_oid.clone()),
            dec!(5.0),          // filled_qty
            Some(dec!(3000.0)), // avg_price
            Some(dec!(1.5)),    // fee
        )
        .await?;

        // 验证部分成交
        let step1 = repo.find_by_uuid(order_uuid).await?.unwrap();
        assert!(matches!(step1.status, OrderStatus::PartiallyFilled));
        assert_eq!(step1.exchange_order_id, Some(exchange_oid.clone()));
        assert_eq!(step1.filled_quantity.0, dec!(5.0));
        assert_eq!(step1.average_price.unwrap().0, dec!(3000.0));
        assert_eq!(step1.fee, Some(dec!(1.5)));

        // 3. 模拟完全成交 (Filled)
        // 剩余 5 个也成交了，总成交 10 个，均价拉平到 3010
        repo.update_status(
            order_uuid,
            OrderStatus::Filled,
            None,       // 交易所ID不变，传None
            dec!(10.0), // total filled
            Some(dec!(3010.0)),
            Some(dec!(3.0)), // total fee
        )
        .await?;

        // 验证完全成交
        let step2 = repo.find_by_uuid(order_uuid).await?.unwrap();
        assert!(matches!(step2.status, OrderStatus::Filled));
        assert_eq!(step2.filled_quantity.0, dec!(10.0));
        assert_eq!(step2.fee, Some(dec!(3.0)));
        // 确保 exchange_order_id 没有被 None 覆盖掉 (COALESCE 逻辑)
        assert_eq!(step2.exchange_order_id, Some(exchange_oid));

        Ok(())
    }

    // =========================================================================
    // 3. 列表查询：按策略查找
    // =========================================================================
    #[tokio::test]
    async fn test_find_by_strategy() -> Result<()> {
        let repo = get_test_repo().await;
        let strategy_uuid_str = Uuid::new_v4().to_string();

        // 1. 插入 3 个订单
        // 订单 A (最早)
        let mut o1 = Order::new_limit(
            "SOL/USDT",
            Exchange::Binance,
            Some(strategy_uuid_str.clone()),
            Side::Buy,
            Price(dec!(20.0)),
            Quantity(dec!(1.0)),
        );
        // 手动微调时间，确保排序可测 (虽然 new 里面是 Now，但执行太快可能相等)
        // 这里依赖数据库的 auto-update 或者插入顺序，但在高并发下最好有不同时间
        // 简单的办法是按顺序插入，依赖 gmt_create

        // 订单 B
        let o2 = Order::new_limit(
            "SOL/USDT",
            Exchange::Binance,
            Some(strategy_uuid_str.clone()),
            Side::Sell,
            Price(dec!(25.0)),
            Quantity(dec!(1.0)),
        );

        // 订单 C (干扰项：属于另一个策略)
        let other_strategy = Uuid::new_v4().to_string();
        let o3 = Order::new_limit(
            "SOL/USDT",
            Exchange::Binance,
            Some(other_strategy),
            Side::Buy,
            Price(dec!(10.0)),
            Quantity(dec!(1.0)),
        );

        repo.insert(&o1).await?;
        // 稍微 sleep 一下保证 gmt_create 不同 (如果是毫秒级数据库)
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        repo.insert(&o2).await?;
        repo.insert(&o3).await?;

        // 2. 查询指定策略的订单
        let strategy_uuid = Uuid::from_str(&strategy_uuid_str)?;
        let orders = repo.find_by_strategy(strategy_uuid).await?;

        // 3. 验证
        // 应该只找到 2 个，o3 不应该在里面
        assert_eq!(orders.len(), 2);

        // 验证排序: SQL 是 ORDER BY gmt_create DESC (最新的在前)
        // 所以 orders[0] 应该是 o2 (后插入的), orders[1] 是 o1
        assert_eq!(orders[0].uuid, o2.uuid);
        assert_eq!(orders[1].uuid, o1.uuid);

        Ok(())
    }
}
