use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

use crate::enums::{OrderStatus, OrderType, Side};
use crate::primitive::CurrencyPair;
use crate::primitive::{Price, Quantity};
use crate::Exchange;
use std::str::FromStr;

/// 订单实体 (Order Entity)
///
/// 对应数据库表: `orders` (建议表名复数形式)
///
/// 该结构体代表了系统中的一张“订单”。它是交易的核心载体，记录了从“想买”到“成交”的全过程。
/// 包含了交易所的原始信息（symbol, exchange_order_id）以及系统的内部状态（status, filled_quantity）。
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Order {
    /// 数据库物理主键 (自增 ID)
    /// 类型: BIGINT (u64)
    /// 作用: 仅用于数据库内部 B+ 树索引和分页优化。
    /// 注意: 业务逻辑层不应使用此 ID，请认准 `uuid`。
    #[sqlx(rename = "id")]
    #[serde(skip)] // 序列化日志/API时通常隐藏内部物理ID
    pub id: i64,

    /// 订单业务唯一标识 (UUID)
    /// 对应数据库字段: `order_uuid`
    /// 作用: 系统全局唯一的订单号。用于日志追踪、状态更新和幂等性校验。
    #[sqlx(rename = "order_uuid")]
    pub uuid: String,

    /// 归属策略 UUID (外键)
    /// 对应数据库字段: `strategy_uuid`
    /// 作用: 标识该订单是由哪个策略发出的。如果为 `None`，代表是人工手动下单或风控强平单。
    #[sqlx(rename = "strategy_uuid")]
    pub strategy_uuid: Option<String>,

    /// 交易所返回的订单 ID
    /// 作用: 用于后续撤单或查询订单状态时与交易所 API 交互。
    /// 初始状态可能为 None (尚未发送到交易所)，发送成功后更新。
    pub exchange_order_id: Option<String>,

    /// 交易标的
    /// 类型: Struct { base: "BTC", quote: "USDT" }
    /// 数据库存储: VARCHAR ("BTC/USDT")
    pub symbol: CurrencyPair,

    /// 交易所名称
    /// 类型: Enum (Binance, Okx...)
    /// 数据库存储: VARCHAR ("BINANCE")
    /// ⚠️ 修改: String -> Exchange
    pub exchange: Exchange,

    /// 买卖方向 (Buy/Sell)
    pub side: Side,

    /// 订单类型 (Limit/Market/Stop...)
    pub order_type: OrderType,

    /// 订单当前状态
    /// 示例: Created, New, PartiallyFilled, Filled, Canceled
    pub status: OrderStatus,

    /// 委托价格
    /// 逻辑: 市价单 (Market) 此字段为 None。
    pub price: Option<Price>,

    /// 委托数量
    /// 作用: 想要购买或出售的基础资产数量。
    pub quantity: Quantity,

    /// 已成交数量
    /// 作用: 随着成交推送不断累加。当 filled_quantity == quantity 时，状态变为 Filled。
    #[sqlx(default)]
    pub filled_quantity: Quantity,

    /// 成交均价
    /// 作用: 只有成交后才有值。
    pub average_price: Option<Price>,

    /// 交易手续费
    /// 注意: 这里直接使用 Decimal，因为手续费币种不确定（可能是 BNB 也可能是 USDT）。
    pub fee: Option<Decimal>,

    /// 创建时间 (gmt_create)
    pub gmt_create: DateTime<Utc>,

    /// 最后修改时间 (gmt_modified)
    pub gmt_modified: DateTime<Utc>,
}

impl Order {
    /// 创建一个新的限价单 (Limit Order)
    ///
    /// 注意：此方法只在内存中创建对象，并未持久化到数据库。
    /// 物理 ID (`db_id`) 默认为 0，只有插入数据库后才有实际意义。
    pub fn new_limit(
        symbol: impl Into<String>,
        exchange: Exchange,
        strategy_uuid: Option<String>,
        side: Side,
        price: Price,
        quantity: Quantity,
    ) -> Self {
        // 解析 Symbol
        let symbol_str: String = symbol.into();
        let pair = CurrencyPair::from_str(&symbol_str)
            .expect("Invalid symbol format for Position (expected BASE/QUOTE)");

        Self {
            id: 0,                // 占位符
            uuid: Uuid::new_v4().to_string(), // 生成业务 UUID
            strategy_uuid,
            exchange_order_id: None,
            symbol: pair,
            exchange,
            side,
            order_type: OrderType::Limit,
            status: OrderStatus::Created,
            price: Some(price),
            quantity,
            filled_quantity: Quantity::ZERO,
            average_price: None,
            fee: None,
            gmt_create: Utc::now(),
            gmt_modified: Utc::now(),
        }
    }

    /// 创建一个新的市价单 (Market Order)
    pub fn new_market(
        symbol: impl Into<String>,
        exchange: Exchange,
        strategy_uuid: Option<String>,
        side: Side,
        quantity: Quantity,
    ) -> Self {
        // 解析 Symbol
        let symbol_str: String = symbol.into();
        let pair = CurrencyPair::from_str(&symbol_str)
            .expect("Invalid symbol format for Position (expected BASE/QUOTE)");
        Self {
            id: 0,
            uuid: Uuid::new_v4().to_string(),
            strategy_uuid,
            exchange_order_id: None,
            symbol: pair,
            exchange,
            side,
            order_type: OrderType::Market,
            status: OrderStatus::Created,
            price: None, // 市价单无价格
            quantity,
            filled_quantity: Quantity::ZERO,
            average_price: None,
            fee: None,
            gmt_create: Utc::now(),
            gmt_modified: Utc::now(),
        }
    }
}
