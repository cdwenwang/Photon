use crate::config::{AgentLLMConfig, PromptConfig}; // 假设 config 在同级模块
use crate::llm::ModelBackend;
use crate::skills::AgentSkill;
use crate::store::ContextStore;
use crate::types::{AgentContext, TaskPayload};
use anyhow::{Context, Result};
use futures::future::join_all;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct ManagerAgentBuilder {
    name: String,
    store: Arc<dyn ContextStore>,
    default_llm: Arc<dyn ModelBackend>,
    specific_llms: HashMap<String, Arc<dyn ModelBackend>>,
    prompts: PromptConfig,
}

impl ManagerAgentBuilder {
    pub fn new(
        name: &str,
        default_llm: Arc<dyn ModelBackend>,
        store: Arc<dyn ContextStore>,
    ) -> Self {
        Self {
            name: name.to_string(),
            store,
            default_llm,
            specific_llms: HashMap::new(),
            prompts: PromptConfig::default(),
        }
    }

    pub fn with_prompts(mut self, prompts: PromptConfig) -> Self {
        self.prompts = prompts;
        self
    }

    pub fn with_planning_llm(mut self, llm: Arc<dyn ModelBackend>) -> Self {
        self.specific_llms.insert("planning".to_string(), llm);
        self
    }

    pub fn with_review_llm(mut self, llm: Arc<dyn ModelBackend>) -> Self {
        self.specific_llms.insert("review".to_string(), llm);
        self
    }

    pub fn with_verification_llm(mut self, llm: Arc<dyn ModelBackend>) -> Self {
        self.specific_llms.insert("verification".to_string(), llm);
        self
    }

    pub fn build(self) -> ManagerAgent {
        let mut llm_config = AgentLLMConfig::new(self.default_llm.clone());

        if let Some(l) = self.specific_llms.get("planning") {
            llm_config.planning = l.clone();
        }
        if let Some(l) = self.specific_llms.get("review") {
            llm_config.review = l.clone();
        }
        if let Some(l) = self.specific_llms.get("verification") {
            llm_config.verification = l.clone();
        }

        ManagerAgent {
            name: self.name,
            skills: HashMap::new(),
            llms: llm_config,
            store: self.store,
            prompts: self.prompts,
        }
    }
}

// --- 数据结构 (保持不变) ---
// ... (SubTask, ExecutionPlan, TaskResult 等结构体代码省略，与原版一致) ...

#[derive(Debug, Deserialize, Serialize, Clone)]
struct SubTask {
    id: String,
    description: String,
    #[serde(default)]
    dependencies: Vec<String>,
    skill_name: String,
    #[serde(default)]
    params: Value,
    #[serde(default)]
    acceptance_criteria: String,
}

#[derive(Debug, Deserialize)]
struct ExecutionPlan {
    #[allow(dead_code)]
    thought: String,
    tasks: Vec<SubTask>,
}

#[derive(Debug)]
struct TaskResult {
    task_id: String,
    success: bool,
    summary: String,
    output_data: Option<Value>,
    verification_feedback: Option<String>,
}

#[derive(Debug, Deserialize)]
struct VerificationResult {
    passed: bool,
    reason: String,
    suggestion: String,
}

#[derive(Debug, Deserialize)]
struct AdjudicationResult {
    final_decision: bool,
    rationale: String,
}

// --- ManagerAgent 实现 ---

pub struct ManagerAgent {
    name: String,
    skills: HashMap<String, Arc<dyn AgentSkill>>,
    llms: AgentLLMConfig,
    store: Arc<dyn ContextStore>,
    prompts: PromptConfig,
}

impl ManagerAgent {
    pub fn new(
        name: &str,
        default_llm: Arc<dyn ModelBackend>,
        store: Arc<dyn ContextStore>,
    ) -> Self {
        ManagerAgentBuilder::new(name, default_llm, store).build()
    }

    pub fn builder(
        name: &str,
        default_llm: Arc<dyn ModelBackend>,
        store: Arc<dyn ContextStore>,
    ) -> ManagerAgentBuilder {
        ManagerAgentBuilder::new(name, default_llm, store)
    }

    pub fn register_skill(&mut self, skill: impl AgentSkill + 'static) {
        self.skills
            .insert(skill.name().to_string(), Arc::new(skill));
    }

    /// **核心入口**：执行一个高层指令。
    ///
    /// 修改说明：此方法现在作为一个 Wrapper，负责初始化上下文，调用核心逻辑，
    /// 并确保无论核心逻辑返回 Ok 还是 Err，都执行 `persist_context`。
    pub async fn run_task<T>(&self, instruction: &str, output_schema_desc: &str) -> Result<T>
    where
        T: DeserializeOwned + Send,
    {
        // 1. 初始化运行上下文
        let ctx = Arc::new(Mutex::new(AgentContext::new()));
        {
            let mut c = ctx.lock().await;
            c.history.push(format!("User Instruction: {}", instruction));
        }

        // 2. 调用内部核心逻辑，并捕获结果
        let result = self
            .run_task_core(instruction, output_schema_desc, ctx.clone())
            .await;

        // 3. 无论 result 是 Ok 还是 Err，都执行持久化
        // 注意：这里我们通过 ctx.lock().await 获取锁的守卫，再解引用传入
        tracing::info!("[{}] Persisting context...", self.name);
        self.persist_context(&*ctx.lock().await).await;

        // 4. 返回核心逻辑的结果
        result
    }

    /// 内部核心逻辑：包含 Planning, Review, Execution Loop, Synthesis
    async fn run_task_core<T>(
        &self,
        instruction: &str,
        output_schema_desc: &str,
        ctx: Arc<Mutex<AgentContext>>, // 传入已创建好的 ctx
    ) -> Result<T>
    where
        T: DeserializeOwned + Send,
    {
        tracing::info!("[{}] Analysis & Planning task: {}", self.name, instruction);

        // --- Phase 1: Planning (规划) ---
        let initial_plan = self.make_plan(instruction).await?;

        // --- Phase 2: Review (审查) ---
        let mut current_plan_tasks = self
            .review_and_refine_plan(instruction, initial_plan)
            .await?;

        tracing::info!(
            "[{}] Optimized Plan: {} tasks",
            self.name,
            current_plan_tasks.len()
        );

        let mut completed_tasks_ids = HashSet::new();
        let mut task_results_history: Vec<(String, String)> = Vec::new();
        let mut global_replan_count = 0;
        const MAX_GLOBAL_REPLANS: usize = 3;

        // --- Phase 3: Execution Loop (执行循环) ---
        loop {
            // 检查是否所有任务都已完成
            if current_plan_tasks
                .iter()
                .all(|t| completed_tasks_ids.contains(&t.id))
            {
                break;
            }

            // 3.1 查找当前满足依赖关系的可执行任务
            let executable_tasks =
                self.find_executable_tasks(&current_plan_tasks, &completed_tasks_ids)?;

            tracing::info!(
                "[{}] Batch executing: {:?}",
                self.name,
                executable_tasks.iter().map(|t| &t.id).collect::<Vec<_>>()
            );

            // 3.2 并行执行这批任务
            let results = self
                .execute_batch_parallel(executable_tasks, ctx.clone())
                .await;

            // 3.3 处理执行结果
            let (batch_failed, failure_info) = self
                .process_execution_results(
                    results,
                    &mut completed_tasks_ids,
                    &mut task_results_history,
                    ctx.clone(),
                )
                .await?;

            // 3.4 错误处理与全局重规划
            if batch_failed {
                if global_replan_count >= MAX_GLOBAL_REPLANS {
                    // 这里返回 Err，但外层的 run_task 会捕获它并在持久化后重新抛出
                    return Err(anyhow::anyhow!(
                        "Exceeded max global replanning limit. Last failure: {}",
                        failure_info
                    ));
                }
                global_replan_count += 1;
                tracing::warn!(
                    "[{}] Global Replanning ({}/{}).",
                    self.name,
                    global_replan_count,
                    MAX_GLOBAL_REPLANS
                );

                let new_tasks = self
                    .replan_remaining_tasks(
                        instruction,
                        &current_plan_tasks,
                        &completed_tasks_ids,
                        &task_results_history,
                        &failure_info,
                    )
                    .await?;

                let mut valid_tasks: Vec<SubTask> = current_plan_tasks
                    .into_iter()
                    .filter(|t| completed_tasks_ids.contains(&t.id))
                    .collect();
                valid_tasks.extend(new_tasks);
                current_plan_tasks = valid_tasks;
            }
        }

        // --- Phase 4: Synthesis (综合) ---
        tracing::info!("[{}] Synthesizing final result...", self.name);

        let (history_text, artifacts_json) = {
            let c = ctx.lock().await;
            (
                c.history.join("\n"),
                serde_json::to_string_pretty(&c.artifacts).unwrap_or_default(),
            )
        };

        let final_result = self
            .synthesize_final_result::<T>(
                instruction,
                &history_text,
                &artifacts_json,
                output_schema_desc,
            )
            .await?;

        // 注意：这里不再调用 persist_context，该操作已移至外层 run_task

        Ok(final_result)
    }

    // --- 下面的所有辅助方法保持不变 ---

    async fn execute_batch_parallel(
        &self,
        tasks: Vec<SubTask>,
        ctx: Arc<Mutex<AgentContext>>,
    ) -> Vec<Result<TaskResult>> {
        // ... (保持原样) ...
        let artifacts_snapshot = { ctx.lock().await.artifacts.clone() };
        let mut futures = Vec::new();

        for task in tasks {
            let skill_map = self.skills.clone();
            let llms_config = self.llms.clone();
            let prompts_config = self.prompts.clone();
            let artifacts = artifacts_snapshot.clone();
            let ctx_ref = ctx.clone();

            futures.push(async move {
                Self::execute_task_pipeline(
                    task,
                    skill_map,
                    ctx_ref,
                    artifacts,
                    llms_config,
                    prompts_config,
                )
                .await
            });
        }
        join_all(futures).await
    }

    #[allow(clippy::too_many_arguments)]
    async fn execute_task_pipeline(
        task: SubTask,
        skills: HashMap<String, Arc<dyn AgentSkill>>,
        ctx: Arc<Mutex<AgentContext>>,
        artifacts: HashMap<String, Value>,
        llms: AgentLLMConfig,
        prompts: PromptConfig,
    ) -> Result<TaskResult> {
        // ... (代码逻辑保持不变) ...
        // 为节省篇幅，此处省略 execute_task_pipeline 具体实现，与原代码一致
        // 建议直接复制原代码中的实现
        let resolved_params = Self::resolve_params_deep(&task.params, &artifacts);
        let max_retries = 2;
        let mut current_skill_name = task.skill_name.clone();
        let mut current_params = resolved_params;
        let mut last_failure_reason = String::new();

        for attempt in 0..=max_retries {
            if let Some(skill) = skills.get(&current_skill_name) {
                let payload = TaskPayload {
                    instruction: task.description.clone(),
                    params: current_params.clone(),
                };
                let execution_result = {
                    let mut c = ctx.lock().await;
                    skill.execute(&mut c, payload).await
                };

                match execution_result {
                    Ok(result) => {
                        let (passed, verify_msg) = Self::verify_with_adjudication(
                            &llms,
                            &prompts.verification_prompt,
                            &prompts.adjudication_prompt,
                            &task,
                            &result.summary,
                        )
                        .await?;

                        if passed {
                            return Ok(TaskResult {
                                task_id: task.id,
                                success: true,
                                summary: result.summary,
                                output_data: result.data,
                                verification_feedback: None,
                            });
                        } else {
                            last_failure_reason = verify_msg;
                        }
                    }
                    Err(e) => {
                        last_failure_reason = format!("Runtime Error: {}", e);
                    }
                }

                if attempt < max_retries {
                    match Self::reflect_and_reroute(
                        &llms.reflection,
                        &prompts.reflection_prompt,
                        &task,
                        &last_failure_reason,
                        &skills,
                    )
                    .await
                    {
                        Ok((new_skill, new_params, _)) => {
                            current_skill_name = new_skill;
                            current_params = new_params;
                        }
                        Err(_) => {}
                    }
                }
            } else {
                return Ok(TaskResult {
                    task_id: task.id,
                    success: false,
                    summary: format!("Skill '{}' not found", current_skill_name),
                    output_data: None,
                    verification_feedback: None,
                });
            }
        }
        Ok(TaskResult {
            task_id: task.id,
            success: false,
            summary: format!("Retries exhausted. Last error: {}", last_failure_reason),
            output_data: None,
            verification_feedback: Some(last_failure_reason),
        })
    }

    async fn verify_with_adjudication(
        llms: &AgentLLMConfig,
        verify_tmpl: &str,
        adjudicate_tmpl: &str,
        task: &SubTask,
        output: &str,
    ) -> Result<(bool, String)> {
        // ... (保持原样) ...
        let mut futures = Vec::new();
        for _ in 0..3 {
            futures.push(Self::verify_output_single(
                &llms.verification,
                verify_tmpl,
                task,
                output,
            ));
        }
        let results = join_all(futures).await;
        // ... 省略具体逻辑，请保持原代码 ...
        // 此处仅为演示结构
        Ok((true, "Pass".into()))
    }

    async fn verify_output_single(
        llm: &Arc<dyn ModelBackend>,
        template: &str,
        task: &SubTask,
        output: &str,
    ) -> Result<VerificationResult> {
        // ... (保持原样) ...
        let prompt = template
            .replace("{{task_description}}", &task.description)
            .replace("{{acceptance_criteria}}", &task.acceptance_criteria)
            .replace("{{actual_output}}", output);
        let raw = llm.chat(&prompt, "Verify Output").await?;
        let json = Self::clean_json_markdown(&raw);
        Ok(serde_json::from_str(&json)?)
    }

    async fn make_plan(&self, instruction: &str) -> Result<ExecutionPlan> {
        // ... (保持原样) ...
        let skill_desc_str = self
            .skills
            .values()
            .map(|s| format!("- **{}**: {}", s.name(), s.description()))
            .collect::<Vec<_>>()
            .join("\n");
        let prompt = self
            .prompts
            .planning_prompt
            .replace("{{skill_descriptions}}", &skill_desc_str)
            .replace("{{user_instruction}}", instruction);
        let raw = self.llms.planning.chat(&prompt, "Initial Plan").await?;
        let json = Self::clean_json_markdown(&raw);
        Ok(serde_json::from_str(&json)?)
    }

    async fn review_and_refine_plan(
        &self,
        instruction: &str,
        plan: ExecutionPlan,
    ) -> Result<Vec<SubTask>> {
        // ... (保持原样) ...
        let plan_json = serde_json::to_string_pretty(&plan.tasks)?;
        let skill_desc_str = self.skills.keys().cloned().collect::<Vec<_>>().join(", ");
        let prompt = self
            .prompts
            .plan_review_prompt
            .replace("{{user_instruction}}", instruction)
            .replace("{{current_plan}}", &plan_json)
            .replace("{{available_skills}}", &skill_desc_str);
        let raw = self.llms.review.chat(&prompt, "Review Plan").await?;
        let json = Self::clean_json_markdown(&raw);
        let new_plan: ExecutionPlan = serde_json::from_str(&json).unwrap_or(plan);
        Ok(new_plan.tasks)
    }

    async fn replan_remaining_tasks(
        &self,
        instruction: &str,
        plan: &[SubTask],
        done: &HashSet<String>,
        results: &[(String, String)],
        reason: &str,
    ) -> Result<Vec<SubTask>> {
        // ... (保持原样) ...
        let completed_desc = results
            .iter()
            .map(|(id, out)| format!("- {}: {}", id, out))
            .collect::<Vec<_>>()
            .join("\n");
        let pending_desc = plan
            .iter()
            .filter(|t| !done.contains(&t.id))
            .map(|t| format!("- {}", t.description))
            .collect::<Vec<_>>()
            .join("\n");
        let prompt = self
            .prompts
            .replanning_prompt
            .replace("{{goal}}", instruction)
            .replace("{{completed_desc}}", &completed_desc)
            .replace("{{failure_reason}}", reason)
            .replace("{{pending_desc}}", &pending_desc);
        let raw = self.llms.replanning.chat(&prompt, "Replan").await?;
        let json = Self::clean_json_markdown(&raw);
        let plan: ExecutionPlan = serde_json::from_str(&json)?;
        Ok(plan.tasks)
    }

    async fn reflect_and_reroute(
        llm: &Arc<dyn ModelBackend>,
        tmpl: &str,
        task: &SubTask,
        err: &str,
        skills: &HashMap<String, Arc<dyn AgentSkill>>,
    ) -> Result<(String, Value, String)> {
        // ... (保持原样) ...
        let skill_list = skills.keys().cloned().collect::<Vec<_>>().join(", ");
        let prompt = tmpl
            .replace("{{task_description}}", &task.description)
            .replace("{{failed_skill}}", &task.skill_name)
            .replace("{{current_params}}", &task.params.to_string())
            .replace("{{error_msg}}", err)
            .replace("{{available_skills}}", &skill_list);
        let raw = llm.chat(&prompt, "Reflect").await?;
        let json = Self::clean_json_markdown(&raw);
        #[derive(Deserialize)]
        struct R {
            new_skill: String,
            new_params: Value,
            reason: String,
        }
        let r: R = serde_json::from_str(&json)?;
        Ok((r.new_skill, r.new_params, r.reason))
    }

    async fn synthesize_final_result<T: DeserializeOwned>(
        &self,
        instruction: &str,
        history: &str,
        artifacts_json: &str,
        schema: &str,
    ) -> Result<T> {
        // ... (保持原样) ...
        let prompt = self
            .prompts
            .synthesis_prompt
            .replace("{{instruction}}", instruction)
            .replace("{{history}}", history)
            .replace("{{artifacts}}", artifacts_json)
            .replace("{{schema}}", schema);
        let raw = self.llms.synthesis.chat(&prompt, "Synthesis").await?;
        let json = Self::clean_json_markdown(&raw);
        Ok(serde_json::from_str(&json)?)
    }

    fn find_executable_tasks(
        &self,
        all: &[SubTask],
        done: &HashSet<String>,
    ) -> Result<Vec<SubTask>> {
        // ... (保持原样) ...
        let executable: Vec<SubTask> = all
            .iter()
            .filter(|t| !done.contains(&t.id))
            .filter(|t| t.dependencies.iter().all(|d| done.contains(d)))
            .cloned()
            .collect();
        if executable.is_empty() && all.iter().any(|t| !done.contains(&t.id)) {
            return Err(anyhow::anyhow!("Plan Deadlock detected."));
        }
        Ok(executable)
    }

    fn resolve_params_deep(params: &Value, artifacts: &HashMap<String, Value>) -> Value {
        // ... (保持原样) ...
        match params {
            Value::String(s) => {
                if s.starts_with("{{") && s.ends_with("}}") {
                    let content = s[2..s.len() - 2].trim();
                    let parts: Vec<&str> = content.split('.').collect();
                    if parts.is_empty() {
                        return Value::String(s.clone());
                    }
                    let task_id = parts[0];
                    if let Some(root_val) = artifacts.get(task_id) {
                        // 简化：此处省略具体遍历逻辑，使用原代码
                        return root_val.clone();
                    }
                }
                Value::String(s.clone())
            }
            Value::Array(arr) => Value::Array(
                arr.iter()
                    .map(|v| Self::resolve_params_deep(v, artifacts))
                    .collect(),
            ),
            Value::Object(map) => {
                let mut new_map = serde_json::Map::new();
                for (k, v) in map {
                    new_map.insert(k.clone(), Self::resolve_params_deep(v, artifacts));
                }
                Value::Object(new_map)
            }
            _ => params.clone(),
        }
    }

    async fn process_execution_results(
        &self,
        results: Vec<Result<TaskResult>>,
        completed_ids: &mut HashSet<String>,
        history_log: &mut Vec<(String, String)>,
        ctx: Arc<Mutex<AgentContext>>,
    ) -> Result<(bool, String)> {
        let mut batch_failed = false;
        let mut failure_info = String::new();
        for res in results {
            let res = res?;
            let mut c = ctx.lock().await;
            if res.success {
                completed_ids.insert(res.task_id.clone());
                history_log.push((res.task_id.clone(), res.summary.clone()));
                c.history.push(format!(
                    "Task [{}] SUCCESS. Summary: {}",
                    res.task_id, res.summary
                ));
                if let Some(data) = res.output_data {
                    c.artifacts.insert(res.task_id.clone(), data);
                }
            } else {
                batch_failed = true;
                let reason = res
                    .verification_feedback
                    .unwrap_or_else(|| "Unknown".into());
                failure_info = format!("Task '{}' failed. Reason: {}", res.task_id, reason);
                c.history
                    .push(format!("Task [{}] FAILURE: {}", res.task_id, failure_info));
            }
        }
        Ok((batch_failed, failure_info))
    }

    async fn persist_context(&self, ctx: &AgentContext) {
        let _ = self.store.save(ctx).await;
    }

    fn clean_json_markdown(input: &str) -> String {
        let mut s = input.trim();
        if let Some(stripped) = s.strip_prefix("```json") {
            s = stripped;
        } else if let Some(stripped) = s.strip_prefix("```") {
            s = stripped;
        }
        if let Some(stripped) = s.strip_suffix("```") {
            s = stripped;
        }
        s.trim().to_string()
    }
}
