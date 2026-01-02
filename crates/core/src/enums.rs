use serde::{Deserialize, Serialize};
use sqlx::encode::IsNull;
use sqlx::error::BoxDynError;
use sqlx::mysql::{MySql, MySqlValueRef};
use sqlx::{Database, Decode, Encode, Type};
use std::str::FromStr;
use strum_macros::{Display, EnumString}; // 注意：通常引入宏是 strum_macros

// =========================================================================
// 宏定义：批量为枚举实现 MySql 下的 String 转换
// =========================================================================
macro_rules! impl_mysql_string_type {
    ($type:ty) => {
        // 1. Type 实现: 告诉 SQLx 这在数据库里就是 String
        impl Type<MySql> for $type {
            fn type_info() -> <MySql as Database>::TypeInfo {
                <String as Type<MySql>>::type_info()
            }

            fn compatible(ty: &<MySql as Database>::TypeInfo) -> bool {
                <String as Type<MySql>>::compatible(ty)
            }
        }

        // 2. Encode 实现: 写入时转为 String
        impl<'q> Encode<'q, MySql> for $type {
            fn encode_by_ref(&self, buf: &mut Vec<u8>) -> IsNull {
                <String as Encode<MySql>>::encode_by_ref(&self.to_string(), buf)
            }
        }

        // 3. Decode 实现: 读取时先读 String 再 Parse
        impl<'r> Decode<'r, MySql> for $type {
            fn decode(value: MySqlValueRef<'r>) -> Result<Self, BoxDynError> {
                let value_str = <String as Decode<MySql>>::decode(value)?;
                <$type>::from_str(&value_str).map_err(|_| {
                    format!("Unknown value for {}: {}", stringify!($type), value_str).into()
                })
            }
        }
    };
}

// =========================================================================
// 枚举定义 (注意：移除了 derive(Type) 和 #[sqlx] 属性)
// =========================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Display, EnumString)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[strum(serialize_all = "SCREAMING_SNAKE_CASE")] // 确保 .to_string() 输出 "BUY"
pub enum Side {
    Buy,
    Sell,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Display, EnumString)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[strum(serialize_all = "SCREAMING_SNAKE_CASE")]
pub enum OrderType {
    Limit,
    Market,
    StopLoss,
    Ioc, // Immediate or Cancel
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Display, EnumString, Default,
)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[strum(serialize_all = "SCREAMING_SNAKE_CASE")]
pub enum OrderStatus {
    #[default]
    Created,
    Pending,
    New,
    PartiallyFilled,
    Filled,
    Canceled,
    Rejected,
    Expired,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize, Display, EnumString,
)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[strum(serialize_all = "SCREAMING_SNAKE_CASE")]
pub enum StrategyStatus {
    #[default]
    Created,
    Initializing,
    Running,
    Paused,
    Stopping,
    Stopped,
    Error,
}

impl StrategyStatus {
    pub fn is_active(&self) -> bool {
        matches!(self, StrategyStatus::Running | StrategyStatus::Initializing)
    }
    pub fn can_trade(&self) -> bool {
        matches!(self, StrategyStatus::Running)
    }
    pub fn is_finished(&self) -> bool {
        matches!(self, StrategyStatus::Stopped | StrategyStatus::Error)
    }
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize, Display, EnumString,
)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[strum(serialize_all = "SCREAMING_SNAKE_CASE")]
pub enum BarPeriod {
    #[default]
    M1,
    M5,
    M15,
    H1,
    H4,
    D1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Display, EnumString)]
#[strum(serialize_all = "SCREAMING_SNAKE_CASE")]
pub enum Exchange {
    Binance,
    Okx,
    Bybit,
    Coinbase,
    Nasdaq,
    Nyse,
}

// =========================================================================
// 统一应用宏
// =========================================================================

impl_mysql_string_type!(Side);
impl_mysql_string_type!(OrderType);
impl_mysql_string_type!(OrderStatus);
impl_mysql_string_type!(StrategyStatus);
impl_mysql_string_type!(BarPeriod);
impl_mysql_string_type!(Exchange);
