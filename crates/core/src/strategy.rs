use crate::enums::Side;
use crate::primitive::{Price, Quantity};
use crate::StrategyStatus;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::FromRow;
use uuid::Uuid;

/// 策略实例实体 (Strategy Instance Entity)
///
/// 对应数据库表: `strategies`
///
/// 该结构体代表了一个具体的、可运行的策略实例。
/// 注意区分 **策略逻辑 (Logic)** 和 **策略实例 (Instance)**：
/// - **逻辑**: 代码中实现的 `Strategy` trait（如 `GridStrategy` 代码）。
/// - **实例**: 数据库中的这一行记录，包含了参数配置、运行状态和唯一 ID。
///
/// 系统启动时，会从数据库读取此结构体，根据 `class_name` 找到对应的代码逻辑，
/// 并注入 `config` 启动运行。
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Strategy {
    /// 数据库物理主键 (自增 ID)
    /// 类型: BIGINT (u64)
    /// 作用: 数据库内部 B+ 树索引使用，确保高性能分页和查询。
    /// 注意: 业务逻辑层不应依赖此 ID，而应使用 `uuid`。
    #[sqlx(rename = "id")]
    pub id: i64,

    /// 策略业务唯一标识 (UUID)
    /// 类型: VARCHAR(36)
    /// 作用: 系统的全局唯一 ID。所有的订单、信号、日志都会关联此 UUID。
    /// 即使迁移数据库或重置自增 ID，此 UUID 永远不变。
    #[sqlx(rename = "uuid")]
    pub uuid: Uuid,

    /// 策略实例名称 (Human Readable Name)
    /// 示例: "BTC_Grid_V1", "ETH_Trend_Follow_001"
    /// 作用: 用于 UI 展示、日志打印，方便人工识别。要求全局唯一。
    pub name: String,

    /// 策略逻辑类名 (Strategy Class Identifier)
    /// 示例: "SpotGridStrategy", "DualThrustStrategy"
    /// 作用: 核心映射字段。系统根据此字段在 `StrategyFactory` 中查找并实例化对应的 Rust 结构体代码。
    pub class_name: String,

    /// 策略当前生命周期状态
    /// 枚举: Created, Running, Paused, Stopped, Error
    /// 作用: 控制策略引擎是否应该调用该策略的 `on_tick` 或 `on_bar` 方法。
    pub status: StrategyStatus,

    /// 策略静态配置参数
    /// 类型: JSON
    /// 示例: `{ "grid_step": 0.01, "upper_limit": 60000, "k": 1.5 }`
    /// 作用: 灵活存储不同策略所需的非结构化参数。
    /// sqlx 会自动将其映射为 `serde_json::Value`。
    pub config: Value,

    /// 记录创建时间 (MySQL: gmt_create)
    pub gmt_create: DateTime<Utc>,

    /// 记录最后修改时间 (MySQL: gmt_modified)
    /// 作用: 记录策略状态变更或配置修改的时间。
    pub gmt_modified: DateTime<Utc>,
}

/// 策略运行时状态快照 (Strategy Runtime State Snapshot)
///
/// 对应数据库表: `strategy_states`
///
/// 该结构体用于持久化保存策略在运行过程中的**动态变量**。
///
/// ### 设计目的:
/// 1. **崩溃恢复 (Crash Recovery)**: 如果系统宕机或重启，策略可以通过加载此记录，恢复到上一次计算的中间状态（如：上一次的金叉价格、当前累积的仓位、移动止损线位置等），实现“热启动”。
/// 2. **数据解耦**: 将“静态配置”(`Strategy.config`) 与 “动态状态”(`StrategyState.state_data`) 分离。
///
/// ### 使用场景:
/// 通常在策略的 `on_tick` 或 `on_order_update` 处理结束时，或者定期（如每分钟）调用 `save_state` 将内存中的变量 Dump 到此表中。
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct StrategyState {
    /// 数据库物理主键 (自增 ID)
    /// 类型: BIGINT (u64)
    /// 作用: 数据库内部索引使用。
    #[sqlx(rename = "id")]
    pub id: i64,

    /// 关联的策略业务 UUID (Foreign Key)
    /// 对应数据库字段: `strategy_uuid`
    /// 作用: 标识这份状态数据属于哪一个策略实例。通常与 `strategies` 表是一对一 (1:1) 关系。
    #[sqlx(rename = "strategy_uuid")]
    pub uuid: Uuid,

    /// 策略运行时动态数据
    /// 类型: JSON
    /// 示例: `{ "last_ma_cross_price": 50000.5, "current_position": 1.2, "stop_loss_level": 49000 }`
    /// 作用: 存储策略运行过程中产生的、需要在重启后保留的变量。
    /// 由于不同策略需要保存的变量各不相同，因此使用 JSON 格式存储非结构化数据。
    pub state_data: Value,

    /// 记录创建时间
    /// 通常在策略第一次启动时创建。
    pub gmt_create: DateTime<Utc>,

    /// 记录最后更新时间
    /// 作用: 非常关键。表示该状态快照的“新鲜度”。如果时间过久，策略重启时可能需要决定是丢弃旧状态还是尝试恢复。
    pub gmt_modified: DateTime<Utc>,
}

/// 策略生成的交易信号 (Signal)
///
/// 信号是策略逻辑计算的产物，代表策略“想”交易的意图。
/// 注意：信号 != 订单。信号发出后，通常需要经过风控层（Risk Manager）的资金检查和仓位限制，
/// 审核通过后才会转换为真正的订单（Order）发往交易所。
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Signal {
    /// 数据库物理主键 (自增 ID)
    /// 类型: BIGINT (u64)
    /// 作用: 仅用于数据库内部索引、分页和性能优化，业务逻辑中尽量避免依赖此 ID。
    #[sqlx(rename = "id")]
    pub id: i64,

    /// 信号业务唯一标识 (UUID)
    /// 类型: VARCHAR(36)
    /// 作用: 全局唯一的业务 ID，用于日志追踪、幂等性检查和分布式关联。
    #[sqlx(rename = "uuid")]
    pub uuid: Uuid,

    /// 归属策略的业务 UUID
    /// 作用: 标识该信号是由哪个策略实例 (Strategy Instance) 计算产生的。
    #[sqlx(rename = "strategy_uuid")]
    pub strategy_uuid: Uuid,

    /// 交易标的 / 交易对
    /// 格式: 标准化格式，如 "BTC/USDT", "ETH-PERP"
    pub symbol: String,

    /// 交易方向
    /// 枚举: Buy (做多/买入) 或 Sell (做空/卖出)
    pub side: Side,

    /// 建议成交价格 (限价)
    /// 含义: 策略希望成交的预期价格。
    /// 逻辑: 如果为 `None`，通常隐含建议以 **市价 (Market Order)** 执行。
    pub price: Option<Price>,

    /// 建议成交数量
    /// 含义: 策略计算出的目标下单量。
    /// 逻辑: 如果为 `None`，则表示策略只发出方向信号，具体数量需由执行层的资金管理模块（Position Sizing）决定。
    pub quantity: Option<Quantity>,

    /// 信号触发原因 / 标签
    /// 作用: 用于回测分析和实盘日志，记录为什么触发该信号。
    /// 示例: "RSI_Oversold", "Golden_Cross", "Grid_Level_5"
    pub reason: String,

    /// 记录创建时间 (MySQL: gmt_create)
    /// 作用: 信号产生的时间点
    pub gmt_create: DateTime<Utc>,

    /// 记录修改时间 (MySQL: gmt_modified)
    /// 作用: 记录状态变更时间 (尽管信号通常是不可变的，但保留此字段符合数据库规范)
    pub gmt_modified: DateTime<Utc>,
}

impl Strategy {
    /// 创建一个新的策略实例
    ///
    /// 注意：此方法仅在内存中创建对象，调用 repo.create() 后才会存入数据库。
    pub fn new(name: impl Into<String>, class_name: impl Into<String>, config: Value) -> Self {
        let now = Utc::now();
        Self {
            id: 0, // 数据库自增，初始为0
            uuid: Uuid::new_v4(),
            name: name.into(),
            class_name: class_name.into(),
            status: StrategyStatus::Created, // 默认状态
            // 如果你的 Struct 定义里有 reason 字段，这里要加上 reason: "".to_string(),
            // 如果没有，请忽略
            config,
            gmt_create: now,
            gmt_modified: now,
        }
    }

    /// 辅助方法：检查策略是否正在运行
    pub fn is_running(&self) -> bool {
        self.status == StrategyStatus::Running
    }

    /// 辅助方法：检查策略是否可以被调度（运行或初始化中）
    pub fn is_active(&self) -> bool {
        matches!(
            self.status,
            StrategyStatus::Running | StrategyStatus::Initializing
        )
    }
}

impl Signal {
    /// 创建一个限价交易信号
    pub fn new_limit(
        strategy_uuid: Uuid,
        symbol: impl Into<String>,
        side: Side,
        price: Price,
        quantity: Quantity,
        reason: impl Into<String>,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: 0,
            uuid: Uuid::new_v4(),
            strategy_uuid,
            symbol: symbol.into(),
            side,
            price: Some(price),
            quantity: Some(quantity),
            reason: reason.into(),
            gmt_create: now,
            gmt_modified: now,
        }
    }

    /// 创建一个市价交易信号 (不带价格)
    pub fn new_market(
        strategy_uuid: Uuid,
        symbol: impl Into<String>,
        side: Side,
        quantity: Quantity,
        reason: impl Into<String>,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: 0,
            uuid: Uuid::new_v4(),
            strategy_uuid,
            symbol: symbol.into(),
            side,
            price: None, // 市价信号没有价格
            quantity: Some(quantity),
            reason: reason.into(),
            gmt_create: now,
            gmt_modified: now,
        }
    }
}

impl StrategyState {
    pub fn new(strategy_uuid: Uuid, state_data: Value) -> Self {
        let now = Utc::now();
        Self {
            id: 0,
            uuid: strategy_uuid, // 注意：这里是外键
            state_data,
            gmt_create: now,
            gmt_modified: now,
        }
    }
}
