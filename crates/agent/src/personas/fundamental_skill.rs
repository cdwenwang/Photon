use crate::llm::ModelBackend;
use crate::personas::AgentSkill;
use crate::{AgentContext, TaskPayload, TaskResult};
use anyhow::Result;
use async_trait::async_trait;
use regex::Regex;
use serde_json::{json, Value}; // 需要在 Cargo.toml 添加 regex

/// 基础面分析 Agent
pub struct FundamentalSkill {
    name: String,
    description: String,
    prompt: String,
    llm: Box<dyn ModelBackend>,
}

impl FundamentalSkill {
    pub fn new(llm: Box<dyn ModelBackend>) -> Self {
        Self {
            name: "行情基本面分析师".to_string(),
            description: include_str!("../../prompts/fundamental_skill_desc.md")
                .trim()
                .to_string(),
            // 假设这里加载了上面修改后的 Prompt
            prompt: include_str!("../../prompts/fundamental_skill.md")
                .trim()
                .to_string(),
            llm,
        }
    }

    /// 辅助函数：从 LLM 的回复中提取 JSON
    /// 处理逻辑：寻找 ```json ... ``` 包裹的内容
    fn extract_json_from_response(&self, content: &str) -> (String, Option<Value>) {
        // 1. 定义正则匹配 Markdown JSON 代码块
        let re = Regex::new(r"(?s)```json\s*(.*?)\s*```").unwrap();

        if let Some(caps) = re.captures(content) {
            let json_str = caps.get(1).map_or("", |m| m.as_str());

            // 尝试解析 JSON
            match serde_json::from_str::<Value>(json_str) {
                Ok(data) => {
                    // 将 JSON 代码块从原文中移除，让 summary 更干净（可选）
                    let summary = re.replace(content, "").to_string();
                    return (summary.trim().to_string(), Some(data));
                }
                Err(e) => {
                    tracing::warn!("Failed to parse JSON from LLM response: {}", e);
                    // 解析失败，返回原始内容和 None
                    return (content.to_string(), None);
                }
            }
        }

        // 2. 如果没找到代码块，尝试直接寻找 '{' 和 '}' (以防 LLM 没加 markdown 标记)
        if let (Some(start), Some(end)) = (content.find('{'), content.rfind('}')) {
            if start < end {
                let potential_json = &content[start..=end];
                if let Ok(data) = serde_json::from_str::<Value>(potential_json) {
                    // 简单处理：保留全文作为 summary
                    return (content.to_string(), Some(data));
                }
            }
        }

        // 3. 实在找不到 JSON
        (content.to_string(), None)
    }
}

#[async_trait]
impl AgentSkill for FundamentalSkill {
    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> &str {
        &self.description
    }

    async fn execute(&self, ctx: &mut AgentContext, payload: TaskPayload) -> Result<TaskResult> {
        // 1. 解析参数
        let topic = payload
            .params
            .get("topic")
            .and_then(|v| v.as_str())
            .unwrap_or("Unknown Asset");

        // 获取上下文历史
        let context_str = ctx.history.join("\n");

        // 2. 构造 Prompt (动态填入参数)
        let system_prompt = self
            .prompt
            .replace("{{topic}}", topic)
            .replace("{{context}}", &context_str);

        // 3. 调用 LLM
        // 注意：这里我们期望 LLM 返回包含 JSON 的完整文本
        let raw_response = self.llm.chat(&system_prompt, self.description()).await?;

        // 4. 解析结果 (核心修改：不再使用硬编码数据)
        let (summary, extracted_data) = self.extract_json_from_response(&raw_response);

        // 如果没有提取到数据，我们可以给一个默认的空值或者错误提示数据
        let final_data = extracted_data.or_else(|| {
            Some(json!({
                "error": "Failed to generate structured data",
                "raw_response": raw_response
            }))
        });

        Ok(TaskResult {
            summary,
            data: final_data,
        })
    }
}
