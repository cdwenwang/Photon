use crate::llm::ModelBackend;
use crate::skills::AgentSkill;
use crate::store::ContextStore;
use crate::types::{AgentContext, TaskPayload};
use anyhow::{Context, Result};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

// --- 数据结构 (保持不变) ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DebateTurn {
    pub round: usize,
    pub speaker: String,
    pub instruction: String,
    pub content: String,
    pub artifacts: Option<Value>,
}

#[derive(Debug, Deserialize)]
struct HostDecision {
    action: String,
    next_speaker: Option<String>,
    instruction: Option<String>,
    rationale: String,
}

// --- Builder (保持不变) ---

pub struct DebateHostBuilder {
    name: String,
    store: Arc<dyn ContextStore>,
    host_llm: Arc<dyn ModelBackend>,
    skills: HashMap<String, Arc<dyn AgentSkill>>,
    max_turns: usize,
    host_prompt: Option<String>,
    synthesis_prompt: Option<String>,
}

impl DebateHostBuilder {
    pub fn new(name: &str, host_llm: Arc<dyn ModelBackend>, store: Arc<dyn ContextStore>) -> Self {
        Self {
            name: name.to_string(),
            store,
            host_llm,
            skills: HashMap::new(),
            max_turns: 10,
            host_prompt: None,
            synthesis_prompt: None,
        }
    }

    pub fn register_skill(mut self, skill: impl AgentSkill + 'static) -> Self {
        self.skills
            .insert(skill.name().to_string(), Arc::new(skill));
        self
    }

    pub fn with_max_turns(mut self, max: usize) -> Self {
        self.max_turns = max;
        self
    }

    pub fn with_host_prompt(mut self, prompt: &str) -> Self {
        self.host_prompt = Some(prompt.to_string());
        self
    }

    pub fn with_synthesis_prompt(mut self, prompt: &str) -> Self {
        self.synthesis_prompt = Some(prompt.to_string());
        self
    }

    pub fn build(self) -> DebateHost {
        DebateHost {
            name: self.name,
            store: self.store,
            host_llm: self.host_llm,
            skills: self.skills,
            max_turns: self.max_turns,
            // 注意：这里保留了你代码中的 include_str!
            host_prompt_template: self.host_prompt.unwrap_or_else(|| {
                include_str!("../prompts/host_prompt_template.md")
                    .trim()
                    .to_string()
            }),
            synthesis_prompt_template: self.synthesis_prompt.unwrap_or_else(|| {
                include_str!("../prompts/debate_synthesis_template.md")
                    .trim()
                    .to_string()
            }),
        }
    }
}

// --- DebateHost 实现 ---

pub struct DebateHost {
    name: String,
    store: Arc<dyn ContextStore>,
    host_llm: Arc<dyn ModelBackend>,
    skills: HashMap<String, Arc<dyn AgentSkill>>,
    max_turns: usize,
    host_prompt_template: String,
    synthesis_prompt_template: String,
}

impl DebateHost {
    pub fn builder(
        name: &str,
        host_llm: Arc<dyn ModelBackend>,
        store: Arc<dyn ContextStore>,
    ) -> DebateHostBuilder {
        DebateHostBuilder::new(name, host_llm, store)
    }

    /// **核心入口**：执行辩论任务
    ///
    /// 修改说明：此方法现在是一个 Wrapper，负责资源的初始化和最终的持久化清理，
    /// 无论内部逻辑成功与否。
    pub async fn run_debate<T>(&self, topic: &str, output_schema: &str) -> Result<T>
    where
        T: DeserializeOwned + Send,
    {
        // 1. 初始化会话上下文
        let ctx = Arc::new(Mutex::new(AgentContext::new()));

        // 记录初始 Topic
        {
            let mut c = ctx.lock().await;
            c.history.push(format!("(Debate Topic): {}", topic));
        }

        // 2. 调用核心逻辑 (Core Logic)，并捕获其返回值
        let result = self
            .run_debate_core(topic, output_schema, ctx.clone())
            .await;

        // 3. 无论 Result 是 Ok 还是 Err，都执行持久化
        tracing::info!("[{}] Saving debate context...", self.name);
        if let Err(e) = self.store.save(&*ctx.lock().await).await {
            tracing::error!("[{}] Failed to save context: {}", self.name, e);
        }

        // 4. 返回核心逻辑的结果
        result
    }

    /// **内部核心逻辑**：包含 Loop, Host Decision, Skill Execution, Synthesis
    ///
    /// 将具体的业务逻辑移到这里，保持 `ctx` 引用传入
    async fn run_debate_core<T>(
        &self,
        topic: &str,
        output_schema: &str,
        ctx: Arc<Mutex<AgentContext>>,
    ) -> Result<T>
    where
        T: DeserializeOwned + Send,
    {
        let mut turns: Vec<DebateTurn> = Vec::new();

        tracing::info!("[{}] Open debate on: '{}'", self.name, topic);

        let mut round = 1;
        loop {
            if round > self.max_turns {
                tracing::warn!("[{}] Max turns reached.", self.name);
                break;
            }

            // 1. Host Decision
            let decision = self.ask_host_for_decision(topic, &turns).await?;

            tracing::info!(
                "[{}] Round {}: Action='{}' (Reason: {})",
                self.name,
                round,
                decision.action,
                decision.rationale
            );

            if decision.action == "conclude" {
                break;
            } else if decision.action == "next" {
                let speaker_name = decision.next_speaker.unwrap_or_default();
                // 移除：let clone_speaker_name = speaker_name.clone();  <-- 这里不需要
                let instruction = decision.instruction.unwrap_or_default();

                if let Some(skill) = self.skills.get(&speaker_name) {
                    tracing::info!("   -> Expert [{}] speaking...", speaker_name);

                    let context_summary = self.format_turns_text(&turns);
                    let payload = TaskPayload {
                        instruction: instruction.clone(),
                        params: json!({
                            "topic": topic,
                            "round": round,
                            "context_summary": context_summary,
                            "host_instruction": instruction
                        }),
                    };

                    let mut c_guard = ctx.lock().await;

                    match skill.execute(&mut c_guard, payload).await {
                        Ok(res) => {
                            let turn = DebateTurn {
                                round,
                                // 这里需要所有权，所以现场 clone，不影响原始变量
                                speaker: speaker_name.clone(),
                                instruction,
                                content: res.summary.clone(),
                                artifacts: res.data.clone(),
                            };

                            c_guard.history.push(format!(
                                "[R{} - {}]: {}",
                                round,
                                speaker_name,
                                res.summary // 这里直接使用 speaker_name
                            ));

                            if let Some(data) = res.data {
                                c_guard.artifacts.insert(
                                    format!("{}_r{}", speaker_name, round), // 这里直接使用 speaker_name
                                    data,
                                );
                            }
                            turns.push(turn);
                        }
                        Err(e) => {
                            tracing::error!("   -> Skill failed: {}", e);

                            turns.push(DebateTurn {
                                round,
                                // 这里同样现场 clone
                                speaker: speaker_name.clone(),
                                instruction,
                                content: format!("ERROR: {}", e),
                                artifacts: None,
                            });

                            // 这里之前用的 clone_speaker_name，现在直接用 speaker_name 即可
                            // 因为 format! 宏会自动借用，不会移动所有权
                            c_guard
                                .history
                                .push(format!("[R{} - {}] FAILED: {}", round, speaker_name, e));
                        }
                    }
                } else {
                    tracing::warn!("   -> Host selected unknown speaker: {}", speaker_name);
                }
            } else {
                tracing::warn!("   -> Unknown action: {}", decision.action);
                break;
            }

            round += 1;
        }

        tracing::info!("[{}] Synthesizing result...", self.name);

        let final_result = self
            .synthesize_result::<T>(topic, &turns, output_schema)
            .await?;

        Ok(final_result)
    }

    // --- Private Methods (Helper) ---

    async fn ask_host_for_decision(
        &self,
        topic: &str,
        turns: &[DebateTurn],
    ) -> Result<HostDecision> {
        let skill_list_str = self
            .skills
            .iter()
            .map(|(k, v)| format!("- Name: {}\n  Description: {}", k, v.description()))
            .collect::<Vec<_>>()
            .join("\n");

        let history_text = if turns.is_empty() {
            "No discussion yet.".to_string()
        } else {
            self.format_turns_text(turns)
        };

        // 使用 replace 进行模板变量替换
        let prompt = self
            .host_prompt_template
            .replace("{{topic}}", topic)
            .replace("{{skill_list}}", &skill_list_str)
            .replace("{{history}}", &history_text);

        let raw = self.host_llm.chat(&prompt, "Host Decision").await?;
        let json = self.clean_json(&raw);
        Ok(serde_json::from_str(&json).context("Failed to parse Host Decision")?)
    }

    async fn synthesize_result<T: DeserializeOwned>(
        &self,
        topic: &str,
        turns: &[DebateTurn],
        schema_desc: &str,
    ) -> Result<T> {
        let history_text = self.format_turns_text(turns);

        // 使用 replace 进行模板变量替换
        let prompt = self
            .synthesis_prompt_template
            .replace("{{topic}}", topic)
            .replace("{{history}}", &history_text)
            .replace("{{schema}}", schema_desc);

        let raw = self.host_llm.chat(&prompt, "Synthesis").await?;
        let json = self.clean_json(&raw);
        Ok(serde_json::from_str(&json).context("Failed to parse Final Synthesis")?)
    }

    fn format_turns_text(&self, turns: &[DebateTurn]) -> String {
        turns
            .iter()
            .map(|t| {
                format!(
                    "--- Round {} ---\nSpeaker: {}\nInstruction: {}\nResult: {}\n",
                    t.round, t.speaker, t.instruction, t.content
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn clean_json(&self, input: &str) -> String {
        let mut s = input.trim();
        if let Some(stripped) = s.strip_prefix("```json") {
            s = stripped;
        } else if let Some(stripped) = s.strip_prefix("```") {
            s = stripped;
        }
        if let Some(stripped) = s.strip_suffix("```") {
            s = stripped;
        }
        s.to_string()
    }
}
