// crates/core/src/enums.rs
use serde::{Deserialize, Serialize};
use strum::{Display, EnumString};
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Display, EnumString)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")] // 序列化为 "BUY", "SELL"
#[strum(serialize_all = "SCREAMING_SNAKE_CASE")]
pub enum Side {
    Buy,
    Sell,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Display, EnumString)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum OrderType {
    Limit,
    Market,
    StopLoss,
    Ioc, // Immediate or Cancel
}


#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Display, EnumString, Default)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[strum(serialize_all = "SCREAMING_SNAKE_CASE")]
pub enum OrderStatus {
    #[default]
    Created,        // 本地已创建，未发送
    Pending,        // 已发送，等待交易所确认
    New,            // 交易所已确认挂单
    PartiallyFilled,// 部分成交
    Filled,         // 全部成交
    Canceled,       // 已撤单
    Rejected,       // 拒单
    Expired,        // 过期
}