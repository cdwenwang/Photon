use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use uuid::Uuid;

/// 1. 任务载荷：Manager 派发给 Worker 的具体工单
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskPayload {
    /// 自然语言指令 (给 LLM 看的)
    /// e.g. "分析 BTC 最近 24h 的舆情"
    pub instruction: String,

    /// 结构化参数 (给代码逻辑用的)
    /// Manager 的 LLM 通过 Function Calling 生成这里的参数
    /// e.g. { "source": "twitter", "use_vector_db": true, "top_k": 10 }
    pub params: Value,
}

/// 2. 执行上下文：贯穿一次请求的生命周期
/// 类似于 HTTP 的 Request Context，用于传递记忆和状态
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentContext {
    /// 链路追踪 ID (用于日志串联)
    pub trace_id: Uuid,

    /// 短期记忆 / 对话历史 (Short-term Memory)
    /// 记录了 Manager 和其他 Agent 之前的交互
    pub history: Vec<String>, // 简单起见用 String，实际可用 ChatMessage 结构

    /// 黑板模式 (Blackboard Pattern)
    /// 用于 Agent 之间共享数据，而不需要把大段文本塞进 Prompt
    /// e.g. Researcher 查到的 5000 字新闻存这里，只返回摘要给 Manager
    pub shared_data: HashMap<String, Value>,
}

impl AgentContext {
    pub fn new() -> Self {
        Self {
            trace_id: Uuid::new_v4(),
            history: Vec::new(),
            shared_data: HashMap::new(),
        }
    }
}

/// 3. 任务结果
#[derive(Debug, Serialize, Deserialize)]
pub struct TaskResult {
    /// 给 Manager 看的总结 (Text)
    pub summary: String,

    /// 结构化产出 (Data)
    /// e.g. { "sentiment_score": 0.8, "sources": [...] }
    pub data: Option<Value>,
}
