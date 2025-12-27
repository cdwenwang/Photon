// crates/core/src/market.rs
use crate::primitive::{Price, Quantity};
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

/// 逐笔行情 (Tick)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tick {
    pub symbol: String,
    pub price: Price,
    pub quantity: Quantity, // 有时交易所会推造成该价格的成交量
    pub timestamp_ms: i64,  // 交易所时间戳 (毫秒)
    pub received_at: i64,   // 本地接收时间戳 (延迟监控用)
}

/// K线数据 (OHLCV)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bar {
    pub symbol: String,
    pub open: Price,
    pub high: Price,
    pub low: Price,
    pub close: Price,
    pub volume: Quantity,
    pub start_time: i64,
    pub end_time: i64,
}

/// 核心事件总线的数据包封装
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MarketEvent {
    Tick(Tick),
    Bar(Bar),
    OrderBookL2(OrderBook), // 需要你自己定义 OrderBook
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderBook {
    // 简化版
    pub symbol: String,
    pub bids: Vec<(Price, Quantity)>,
    pub asks: Vec<(Price, Quantity)>,
}