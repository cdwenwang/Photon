use crate::enums::{OrderStatus, OrderType, Side};
use crate::primitive::{Price, Quantity};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// =========================================================================
// Order (标准订单模型)
// =========================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Order {
    /// 本地生成的唯一 ID (用于防止重放、日志追踪)
    pub id: Uuid,

    /// 交易所返回的订单 ID (下单成功前是 None)
    pub exchange_order_id: Option<String>,

    /// 交易对 (e.g., "BTC/USDT")
    pub symbol: String,

    /// 交易所名称 (e.g., "BINANCE", "OKX")
    pub exchange: String,

    /// 买卖方向
    pub side: Side,

    /// 订单类型 (限价、市价等)
    pub order_type: OrderType,

    /// 订单状态
    pub status: OrderStatus,

    /// 委托价格 (市价单为 None)
    pub price: Option<Price>,

    /// 委托数量
    pub quantity: Quantity,

    /// 已成交数量
    pub filled_quantity: Quantity,

    /// 成交均价 (部分成交或全部成交后计算)
    pub average_price: Option<Price>,

    /// 交易手续费 (可选)
    pub fee: Option<Decimal>, // 手续费通常直接用 Decimal，因为它可能是 BNB 也可能是 USDT

    /// 创建时间 (Unix毫秒时间戳) - 对应数据库 BIGINT
    pub created_at: i64,

    /// 更新时间 (Unix毫秒时间戳)
    pub updated_at: i64,
}

impl Order {
    /// 创建一个新的限价单 (Limit Order)
    pub fn new_limit(
        symbol: impl Into<String>,
        exchange: impl Into<String>,
        side: Side,
        price: Price,
        quantity: Quantity,
    ) -> Self {
        let now = Utc::now().timestamp_millis();
        Self {
            id: Uuid::new_v4(),
            exchange_order_id: None,
            symbol: symbol.into(),
            exchange: exchange.into(),
            side,
            order_type: OrderType::Limit,
            status: OrderStatus::Created,
            price: Some(price),
            quantity,
            filled_quantity: Quantity::ZERO,
            average_price: None,
            fee: None,
            created_at: now,
            updated_at: now,
        }
    }

    /// 创建一个新的市价单 (Market Order)
    pub fn new_market(
        symbol: impl Into<String>,
        exchange: impl Into<String>,
        side: Side,
        quantity: Quantity,
    ) -> Self {
        let now = Utc::now().timestamp_millis();
        Self {
            id: Uuid::new_v4(),
            exchange_order_id: None,
            symbol: symbol.into(),
            exchange: exchange.into(),
            side,
            order_type: OrderType::Market,
            status: OrderStatus::Created,
            price: None, // 市价单没有价格
            quantity,
            filled_quantity: Quantity::ZERO,
            average_price: None,
            fee: None,
            created_at: now,
            updated_at: now,
        }
    }

    /// 检查订单是否活跃 (可以撤单)
    pub fn is_active(&self) -> bool {
        matches!(
            self.status,
            OrderStatus::Created | OrderStatus::Pending | OrderStatus::New | OrderStatus::PartiallyFilled
        )
    }

    /// 检查订单是否已结束
    pub fn is_closed(&self) -> bool {
        !self.is_active()
    }

    /// 计算剩余未成交数量
    pub fn remaining_quantity(&self) -> Quantity {
        self.quantity - self.filled_quantity
    }
}

// 为了能在 oms.rs 里用 Decimal，需要引入
use rust_decimal::Decimal;