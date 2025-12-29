use anyhow::Result;
use dotenvy::dotenv;
use quant_core::ensure_that;
use quant_storage::redis::RedisService;
use uuid::Uuid;

async fn service() -> RedisService {
    dotenv().ok();
    let redis_url = std::env::var("REDIS_URL").expect("REDIS missing");
    print!("{}", redis_url);
    RedisService::new(redis_url.as_str()).expect("Failed to initialize RedisService")
}

#[tokio::test]
pub async fn test_redis_basic_flow() {
    let redis_service = service().await;

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

#[tokio::test]
pub async fn test_lock_with_retry() -> Result<()> {
    ensure_that!(1 > 0, "用户ID无效: {}", "id");
    let redis_service = service().await;
    let lock_key = "test_lock_unit_test"; // 换个 Key 避免和旧数据冲突

    // 【关键修复 1】生成并保存 Token，确保加锁和解锁用的是同一个！
    let token = Uuid::new_v4().to_string();

    println!("1. Attempting to lock with token: {}", token);

    // 2. 加锁
    let lock_result = redis_service
        .lock_with_retry(
            lock_key, &token, // 传入 Token 引用
            10000,  // 【关键修复 2】TTL 设为 10秒，防止测试跑慢了自动过期
            5000,   // 等待超时 5秒
            100,    // 重试间隔
        )
        .await
        .expect("Failed to execute lock command");

    assert!(lock_result, "Should acquire lock successfully");
    println!("2. Lock acquired!");

    // 3. 模拟业务处理 (可选)
    // tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    // 4. 解锁 (必须传入同一个 Token)
    let unlock_result = redis_service
        .unlock(lock_key, &token) // <--- 这里必须是同一个变量
        .await
        .expect("Failed to execute unlock command");

    // 如果这里失败，说明 Key 过期了或者 Token 不对
    assert!(
        unlock_result,
        "Should release lock successfully (Token match)"
    );
    println!("3. Lock released!");

    // 5. 再次解锁应该失败 (因为锁已经没了)
    let second_unlock = redis_service.unlock(lock_key, &token).await.unwrap();
    assert!(!second_unlock, "Should fail to unlock a second time");
    Ok(())
}
