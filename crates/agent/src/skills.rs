use crate::{AgentContext, TaskPayload, TaskResult};
use anyhow::Result;
use async_trait::async_trait;

/// 技能接口
#[async_trait]
pub trait AgentSkill: Send + Sync {
    /// 技能名称
    fn name(&self) -> &str;

    /// 技能描述
    fn description(&self) -> &str;

    /// 关键修改：
    /// 1. 引入 ctx: 允许读取历史记忆，或写入共享数据 (由此支持记忆传递)
    /// 2. 引入 payload: 支持结构化入参 (由此支持复杂任务参数)
    async fn execute(&self, ctx: &mut AgentContext, payload: TaskPayload) -> Result<TaskResult>;
}
