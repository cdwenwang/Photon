use crate::llm::ModelBackend;
use crate::personas::AgentSkill;
use crate::{AgentContext, TaskPayload, TaskResult};
use anyhow::Result;
use async_trait::async_trait;
use regex::Regex;
use serde_json::{json, Value};

/// 全球宏观策略 Agent
pub struct MacroSkill {
    name: String,
    description: String,
    prompt: String,
    llm: Box<dyn ModelBackend>,
}

impl MacroSkill {
    pub fn new(llm: Box<dyn ModelBackend>) -> Self {
        Self {
            name: "全球宏观策略师".to_string(),
            // 对应之前的 Macro Persona 描述
            description: include_str!("../../prompts/macro_skill_desc.md")
                .trim()
                .to_string(),
            // 对应之前的 Macro Protocol 提示词
            prompt: include_str!("../../prompts/macro_skill.md")
                .trim()
                .to_string(),
            llm,
        }
    }

    /// 辅助函数：提取 JSON (建议后续抽取为公共 Utils)
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
                    tracing::warn!("Failed to parse JSON from LLM response: {}", e);
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
impl AgentSkill for MacroSkill {
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

        // 宏观分析通常不需要太多的历史细节，更多依赖外部环境
        // 但这里我们将 history 传入作为 Context 补充
        let system_prompt = self
            .prompt
            .replace("{{topic}}", topic)
            .replace("{{context}}", &context_str);

        // Role 描述传给 LLM
        let raw_response = self.llm.chat(&system_prompt, self.description()).await?;

        let (summary, extracted_data) = self.extract_json_from_response(&raw_response);

        let final_data = extracted_data.or_else(|| {
            Some(json!({
                "error": "Failed to generate macro structured data",
                "raw_response": raw_response
            }))
        });

        Ok(TaskResult {
            summary,
            data: final_data,
        })
    }
}
