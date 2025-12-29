pub mod validate {
    /// 核心校验宏：如果条件为假，则返回格式化的错误信息
    #[macro_export]
    macro_rules! ensure_that {
        ($cond:expr, $($arg:tt)+) => {
            if !($cond) {
                return Err(::anyhow::anyhow!($($arg)+));
            }
        };
    }

    /// 校验字符串、Vec、HashMap 等集合不为空
    #[macro_export]
    macro_rules! ensure_not_empty {
        ($container:expr, $($arg:tt)+) => {
            if $container.is_empty() {
                return Err(::anyhow::anyhow!($($arg)+));
            }
        };
    }

    /// 校验 Option 不为 None
    #[macro_export]
    macro_rules! ensure_some {
        ($option:expr, $($arg:tt)+) => {
            if $option.is_none() {
                return Err(::anyhow::anyhow!($($arg)+));
            }
        };
    }

    /// 校验两个值相等
    #[macro_export]
    macro_rules! ensure_eq {
        ($left:expr, $right:expr, $($arg:tt)+) => {
            if $left != $right {
                return Err(::anyhow::anyhow!($($arg)+));
            }
        };
    }

    /// 校验数值在范围内 (包含边界 [min, max])
    #[macro_export]
    macro_rules! ensure_range {
        ($val:expr, $min:expr, $max:expr, $($arg:tt)+) => {
            if $val < $min || $val > $max {
                return Err(::anyhow::anyhow!($($arg)+));
            }
        };
    }
}