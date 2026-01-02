use crate::llm::ModelBackend;
use crate::personas::AgentSkill;
use crate::{AgentContext, TaskPayload, TaskResult};
use anyhow::Result;
use async_trait::async_trait;
use regex::Regex;
use serde_json::{json, Value};

/// 激进做空机构 Agent
pub struct ShortSellerSkill {
    name: String,
    description: String,
    prompt: String,
    llm: Box<dyn ModelBackend>,
}

impl ShortSellerSkill {
    pub fn new(llm: Box<dyn ModelBackend>) -> Self {
        Self {
            name: "激进做空机构调查员".to_string(),
            description: include_str!("../../prompts/short_seller_skill_desc.md")
                .trim()
                .to_string(),
            prompt: include_str!("../../prompts/short_seller_skill.md")
                .trim()
                .to_string(),
            llm,
        }
    }

    fn extract_json_from_response(&self, content: &str) -> (String, Option<Value>) {
        let re = Regex::new(r"(?s)```json\s*(.*?)\s*```").unwrap();
        if let Some(caps) = re.captures(content) {
            let json_str = caps.get(1).map_or("", |m| m.as_str());
            match serde_json::from_str::<Value>(json_str) {
                Ok(data) => {
                    let summary = re.replace(content, "").to_string();
                    return (summary.trim().to_string(), Some(data));
                }
                Err(e) => {
                    // 做空机构通常会输出很长的数据，增加一些特定的 Error log
                    tracing::warn!("ShortSeller JSON Parse Error: {}", e);
                    return (content.to_string(), None);
                }
            }
        }
        if let (Some(start), Some(end)) = (content.find('{'), content.rfind('}')) {
            if start < end {
                if let Ok(data) = serde_json::from_str::<Value>(&content[start..=end]) {
                    return (content.to_string(), Some(data));
                }
            }
        }
        (content.to_string(), None)
    }
}

#[async_trait]
impl AgentSkill for ShortSellerSkill {
    fn name(&self) -> &str {
        &self.name
    }
    fn description(&self) -> &str {
        &self.description
    }

    async fn execute(&self, ctx: &mut AgentContext, payload: TaskPayload) -> Result<TaskResult> {
        let topic = payload
            .params
            .get("topic")
            .and_then(|v| v.as_str())
            .unwrap_or("Unknown Asset");

        // 做空机构特别依赖历史上下文，因为它要攻击其他人的观点
        let context_str = ctx.history.join("\n");

        let system_prompt = self
            .prompt
            .replace("{{topic}}", topic)
            .replace("{{context}}", &context_str);

        // Role 设为 Activist Short Seller
        let raw_response = self.llm.chat(&system_prompt, self.description()).await?;
        let (summary, extracted_data) = self.extract_json_from_response(&raw_response);

        let final_data = extracted_data.or_else(|| {
            Some(json!({
                "error": "Failed to generate short report data",
                "raw_response": raw_response
            }))
        });

        Ok(TaskResult {
            summary,
            data: final_data,
        })
    }
}
