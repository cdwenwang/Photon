use chrono::{DateTime, NaiveDateTime, TimeZone, Utc};
use std::sync::atomic::{AtomicI64, Ordering};

// =========================================================================
// 全局时钟控制 (用于回测)
// =========================================================================

/// 全局模拟时间 (0 表示使用系统真实时间)
/// 使用 AtomicI64 保证线程安全
static MOCK_TIME: AtomicI64 = AtomicI64::new(0);

pub struct Clock;

impl Clock {
    /// 获取当前时间戳 (毫秒)
    ///
    /// 逻辑：如果设置了模拟时间(回测模式)，返回模拟时间；否则返回系统真实时间。
    #[inline]
    pub fn now_ms() -> i64 {
        let mock = MOCK_TIME.load(Ordering::Relaxed);
        if mock > 0 {
            mock
        } else {
            Utc::now().timestamp_millis()
        }
    }

    /// 获取当前时间戳 (微秒)
    #[inline]
    pub fn now_micros() -> i64 {
        let mock = MOCK_TIME.load(Ordering::Relaxed);
        if mock > 0 {
            mock * 1000 // 简单处理：如果回测精度只到毫秒，这里乘1000
        } else {
            Utc::now().timestamp_micros()
        }
    }

    /// 获取当前 UTC 时间对象
    pub fn now() -> DateTime<Utc> {
        let ms = Self::now_ms();
        Self::from_timestamp_ms(ms)
    }

    // -----------------------------------------------------------------
    // 回测专用方法
    // -----------------------------------------------------------------

    /// 设置模拟时间 (用于回测引擎)
    /// 在回测的每一个 Tick/Bar 循环开始时调用它
    pub fn set_mock_time(timestamp_ms: i64) {
        MOCK_TIME.store(timestamp_ms, Ordering::Relaxed);
    }

    /// 重置为系统真实时间 (回测结束时调用)
    pub fn reset() {
        MOCK_TIME.store(0, Ordering::Relaxed);
    }
}

// =========================================================================
// 格式化与转换工具
// =========================================================================

impl Clock {
    /// 时间戳 (ms) -> DateTime<Utc>
    pub fn from_timestamp_ms(ms: i64) -> DateTime<Utc> {
        let seconds = ms / 1000;
        let nsecs = ((ms % 1000) * 1_000_000) as u32;
        // from_timestamp_opt 是 Rust chrono 新版推荐用法，防止溢出
        DateTime::from_timestamp(seconds, nsecs).unwrap_or_default()
    }

    /// 时间戳 (ms) -> 字符串 "2025-01-01 12:00:00"
    pub fn format_ms(ms: i64) -> String {
        let dt = Self::from_timestamp_ms(ms);
        dt.format("%Y-%m-%d %H:%M:%S").to_string()
    }

    /// 字符串 -> 时间戳 (ms)
    /// 支持 "2025-01-01 12:00:00"
    pub fn parse_str(s: &str) -> Option<i64> {
        // 尝试解析常见的几种格式
        let formats = vec![
            "%Y-%m-%d %H:%M:%S",
            "%Y-%m-%dT%H:%M:%S",
            "%Y-%m-%dT%H:%M:%SZ",
        ];

        for fmt in formats {
            if let Ok(dt) = NaiveDateTime::parse_from_str(s, fmt) {
                return Some(dt.and_utc().timestamp_millis());
            }
        }
        None
    }
}

// =========================================================================
// 单元测试
// =========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn test_real_time() {
        Clock::reset(); // 确保是真实模式
        let t1 = Clock::now_ms();
        thread::sleep(Duration::from_millis(10));
        let t2 = Clock::now_ms();
        assert!(t2 >= t1 + 10, "Real time should move forward");
    }
}