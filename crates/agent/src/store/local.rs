use async_trait::async_trait;
use anyhow::Result;
use std::path::PathBuf;
use tokio::fs;
use uuid::Uuid;
use super::ContextStore; // 假设 Trait 定义在 mod.rs
use crate::types::AgentContext;

pub struct LocalFileStore {
    root_dir: PathBuf,
}

impl LocalFileStore {
    /// 初始化存储，如果目录不存在则创建
    pub fn new(path: impl Into<PathBuf>) -> Self {
        let root_dir = path.into();
        // 同步创建目录（仅在程序启动时执行一次，可接受同步）
        if !root_dir.exists() {
            std::fs::create_dir_all(&root_dir)
                .expect("Failed to create local context store directory");
        }
        Self { root_dir }
    }
}

#[async_trait]
impl ContextStore for LocalFileStore {
    async fn save(&self, ctx: &AgentContext) -> Result<()> {
        // 文件名: {trace_id}.json
        let file_name = format!("{}.json", ctx.trace_id);
        let file_path = self.root_dir.join(file_name);

        // 序列化 (Pretty Print 方便人工调试)
        let content = serde_json::to_string_pretty(ctx)?;

        // 异步写入
        fs::write(&file_path, content).await?;
        Ok(())
    }

    async fn load(&self, trace_id: &Uuid) -> Result<Option<AgentContext>> {
        let file_path = self.root_dir.join(format!("{}.json", trace_id));

        if !file_path.exists() {
            return Ok(None);
        }

        // 异步读取
        let content = fs::read_to_string(file_path).await?;
        let ctx: AgentContext = serde_json::from_str(&content)?;

        Ok(Some(ctx))
    }
}