use crate::llm::ModelBackend;
use crate::personas::AgentSkill;
use crate::{AgentContext, TaskPayload, TaskResult};
use anyhow::Result;
use async_trait::async_trait;
use regex::Regex;
use serde_json::{json, Value};

/// 市场情绪与另类数据分析 Agent
pub struct SentimentSkill {
    name: String,
    description: String,
    prompt: String,
    llm: Box<dyn ModelBackend>,
}

impl SentimentSkill {
    pub fn new(llm: Box<dyn ModelBackend>) -> Self {
        Self {
            name: "市场情绪量化分析师".to_string(),
            description: include_str!("../../prompts/sentiment_skill_desc.md")
                .trim()
                .to_string(),
            prompt: include_str!("../../prompts/sentiment_skill.md")
                .trim()
                .to_string(),
            llm,
        }
    }

    fn extract_json_from_response(&self, content: &str) -> (String, Option<Value>) {
        let re = Regex::new(r"(?s)```json\s*(.*?)\s*```").unwrap();
        if let Some(caps) = re.captures(content) {
            let json_str = caps.get(1).map_or("", |m| m.as_str());
            if let Ok(data) = serde_json::from_str::<Value>(json_str) {
                let summary = re.replace(content, "").to_string();
                return (summary.trim().to_string(), Some(data));
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
impl AgentSkill for SentimentSkill {
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

        let context_str = ctx.history.join("\n");

        let system_prompt = self
            .prompt
            .replace("{{topic}}", topic)
            .replace("{{context}}", &context_str);

        let raw_response = self.llm.chat(&system_prompt, self.description()).await?;
        let (summary, extracted_data) = self.extract_json_from_response(&raw_response);

        let final_data = extracted_data.or_else(|| {
            Some(json!({
                "error": "Failed to generate sentiment data",
                "raw_response": raw_response
            }))
        });

        Ok(TaskResult {
            summary,
            data: final_data,
        })
    }
}