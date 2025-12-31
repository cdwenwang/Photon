pub mod local;

use crate::types::AgentContext;
use anyhow::Result;
use async_trait::async_trait;
use uuid::Uuid;

/// 上下文存储接口
///
/// 该接口定义了如何持久化 Agent 的执行状态。
/// 任何实现该接口的结构体（如 LocalFileStore, RedisStore, S3Store）
/// 都可以被注入到 ManagerAgent 中。
#[async_trait]
pub trait ContextStore: Send + Sync {
    /// 保存/更新上下文
    ///
    /// # 参数
    /// * `ctx` - 当前的 Agent 执行上下文，包含 trace_id, history, shared_data 等
    async fn save(&self, ctx: &AgentContext) -> Result<()>;

    /// 读取上下文
    ///
    /// 用于调试、复盘或从中断处恢复任务。
    ///
    /// # 参数
    /// * `trace_id` - 任务的唯一追踪 ID
    ///
    /// # 返回
    /// * `Ok(Some(ctx))` - 找到记录
    /// * `Ok(None)` - 未找到记录 (不是错误)
    /// * `Err(e)` - 读取过程出错 (如文件权限、网络断连)
    async fn load(&self, trace_id: &Uuid) -> Result<Option<AgentContext>>;
}
