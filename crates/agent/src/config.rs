use std::sync::Arc;
use crate::llm::ModelBackend;

/// --- Prompt 配置中心 ---
/// 编译时加载 crates/agent/prompts/ 下的 Markdown 文件
#[derive(Debug, Clone)]
pub struct PromptConfig {
    pub planning_prompt: String,
    pub plan_review_prompt: String,
    pub reflection_prompt: String,
    pub replanning_prompt: String,
    pub synthesis_prompt: String,
    pub verification_prompt: String,
    pub adjudication_prompt: String,
}

impl Default for PromptConfig {
    fn default() -> Self {
        Self {
            // 路径是相对于当前 rust 源文件的
            // 假设当前文件在 crates/agent/src/manager.rs
            // prompts 目录在 crates/agent/prompts/
            planning_prompt: include_str!("../prompts/planning_prompt_template.md").trim().to_string(),
            plan_review_prompt: include_str!("../prompts/plan_review_prompt_template.md").trim().to_string(),
            reflection_prompt: include_str!("../prompts/reflection_prompt_template.md").trim().to_string(),
            replanning_prompt: include_str!("../prompts/replanning_prompt_template.md").trim().to_string(),
            synthesis_prompt: include_str!("../prompts/synthesis_prompt_template.md").trim().to_string(),
            verification_prompt: include_str!("../prompts/verification_prompt_template.md").trim().to_string(),
            adjudication_prompt: include_str!("../prompts/adjudication_prompt_template.md").trim().to_string(),
        }
    }
}

/// --- LLM 环境配置 ---
/// 允许为 Agent 的不同思考阶段配置不同的模型
/// 例如：Planning 用 GPT-4，Verification 用 GPT-3.5-Turbo 以节省成本
#[derive(Clone)]
pub struct AgentLLMConfig {
    pub planning: Arc<dyn ModelBackend>,
    pub review: Arc<dyn ModelBackend>,
    pub reflection: Arc<dyn ModelBackend>,
    pub replanning: Arc<dyn ModelBackend>,
    pub synthesis: Arc<dyn ModelBackend>,
    pub verification: Arc<dyn ModelBackend>,
    pub adjudication: Arc<dyn ModelBackend>,
}

impl AgentLLMConfig {
    /// 创建默认配置，所有阶段使用同一个 Default LLM
    pub fn new(default_llm: Arc<dyn ModelBackend>) -> Self {
        Self {
            planning: default_llm.clone(),
            review: default_llm.clone(),
            reflection: default_llm.clone(),
            replanning: default_llm.clone(),
            synthesis: default_llm.clone(),
            verification: default_llm.clone(),
            adjudication: default_llm.clone(),
        }
    }
}