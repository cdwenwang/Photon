#[cfg(test)]
mod common_repo_test {
    use quant_storage::repository::common;
    use sqlx::Row;
    use tokio;

    #[tokio::test]
    async fn test_dql() {
        let result = common::dql("select count(1) as count from `asset`").await;
        assert!(result.is_ok());
        let vec = result.unwrap();
        assert_eq!(vec.len(), 1);
        let x = vec[0].get::<i64, &str>("count");
        assert!(x > 0);
    }

    #[tokio::test]
    async fn test_dml() {
        let result = common::dml("INSERT INTO photon_db.asset ( uuid, account_name, exchange, currency, free, frozen, borrowed, gmt_create, gmt_modified) VALUES ('a56baf7e-1241-4591-ac20-a22cf928009c', 'test_asset_480339bc-9dfd-4d27-80a0-6217b2c23abc', 'BINANCE', 'USDT', 1000.50000000, 100.00000000, 0.00000000, '2025-12-29 17:57:51.901', '2025-12-29 17:57:51.901');").await.expect("insert asset failed");
        assert_eq!(result, 1);
        common::dml(
            "DELETE FROM photon_db.asset WHERE uuid = 'a56baf7e-1241-4591-ac20-a22cf928009c'",
        )
        .await
        .expect("delete asset failed");
    }
}
