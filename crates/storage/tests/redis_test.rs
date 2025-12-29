use quant_storage::redis;
use uuid::Uuid;

#[tokio::test]
pub async fn test_redis_basic_flow() {
    let redis_service = redis::service().await;

    // 1. 使用随机 Key，防止并行测试时冲突
    let key = format!("test_key:{}", Uuid::new_v4());
    let value = "test_value";

    // 2. 写入数据，并断言写入成功
    // 假设你的 set 方法返回 Result<(), Error>
    let set_result = redis_service.set(&key, value, None).await;
    assert!(
        set_result.is_ok(),
        "Failed to set value to Redis: {:?}",
        set_result.err()
    );

    // 3. 读取数据
    let result = redis_service.get(&key).await;

    // 4. 更优雅的断言
    // 确保没有网络错误
    assert!(result.is_ok(), "Redis connection or query failed");

    // 确保业务逻辑正确（Option 不为 None 且值相等）
    let saved_value = result.unwrap();
    assert_eq!(saved_value, Some(value.to_string()), "Value mismatch");

    // 5. 清理数据 (Teardown)
    // 即使上面断言失败，这一步可能执行不到，但在集成测试中手动调用 del 是个好习惯
    let _ = redis_service.delete(&key).await;
}
