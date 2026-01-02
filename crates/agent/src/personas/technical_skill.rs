use crate::llm::ModelBackend;
use crate::personas::AgentSkill;
use crate::{AgentContext, TaskPayload, TaskResult};
use anyhow::Result;
use async_trait::async_trait;
use regex::Regex;
use serde_json::{json, Value};

/// 技术面分析 Agent
pub struct TechnicalSkill {
    name: String,
    description: String,
    prompt: String,
    llm: Box<dyn ModelBackend>,
}

impl TechnicalSkill {
    pub fn new(llm: Box<dyn ModelBackend>) -> Self {
        Self {
            name: "行情技术面分析师".to_string(),
            // 加载给 Host 看的描述信息
            description: include_str!("../../prompts/technical_skill_desc.md")
                .trim()
                .to_string(),
            // 加载给 LLM 看的系统提示词
            prompt: include_str!("../../prompts/technical_skill.md")
                .trim()
                .to_string(),
            llm,
        }
    }

    /// 辅助函数：从 LLM 的回复中提取 JSON (逻辑与 FundamentalSkill 相同)
    fn extract_json_from_response(&self, content: &str) -> (String, Option<Value>) {
        // 1. 定义正则匹配 Markdown JSON 代码块
        let re = Regex::new(r"(?s)```json\s*(.*?)\s*```").unwrap();

        if let Some(caps) = re.captures(content) {
            let json_str = caps.get(1).map_or("", |m| m.as_str());

            match serde_json::from_str::<Value>(json_str) {
                Ok(data) => {
                    let summary = re.replace(content, "").to_string();
                    return (summary.trim().to_string(), Some(data));
                }
                Err(e) => {
                    tracing::warn!("Failed to parse JSON from LLM response: {}", e);
                    return (content.to_string(), None);
                }
            }
        }

        // 2. 备用逻辑：寻找纯大括号
        if let (Some(start), Some(end)) = (content.find('{'), content.rfind('}')) {
            if start < end {
                let potential_json = &content[start..=end];
                if let Ok(data) = serde_json::from_str::<Value>(potential_json) {
                    return (content.to_string(), Some(data));
                }
            }
        }

        (content.to_string(), None)
    }
}

#[async_trait]
impl AgentSkill for TechnicalSkill {
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

        // 获取上下文历史 (例如基本面分析师的观点，技术面可以反驳)
        let context_str = ctx.history.join("\n");

        // 2. 构造 Prompt
        let system_prompt = self
            .prompt
            .replace("{{topic}}", topic)
            .replace("{{context}}", &context_str);

        // 3. 调用 LLM
        // Role 描述可以是 "Technical Trader"
        let raw_response = self.llm.chat(&system_prompt, self.description()).await?;

        // 4. 解析结果
        let (summary, extracted_data) = self.extract_json_from_response(&raw_response);

        // 构造默认错误数据
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