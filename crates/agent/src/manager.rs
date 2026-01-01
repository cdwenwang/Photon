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

// --- Builder 实现 ---

/// `ManagerAgent` 的构建器。
/// 允许用户配置不同阶段使用的 LLM 模型（例如：规划阶段用强模型，验证阶段用快模型）以及自定义 Prompt。
pub struct ManagerAgentBuilder {
    name: String,
    store: Arc<dyn ContextStore>,
    /// 默认模型，用于未明确指定阶段的兜底 (Fallback)
    default_llm: Arc<dyn ModelBackend>,
    /// 存储特定阶段 (Key) 对应的模型后端 (Value)
    specific_llms: HashMap<String, Arc<dyn ModelBackend>>,
    /// Prompt 配置，包含各个阶段的提示词模板
    prompts: PromptConfig,
}

impl ManagerAgentBuilder {
    /// 创建一个新的构建器实例
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

    /// 替换默认的 Prompt 配置
    pub fn with_prompts(mut self, prompts: PromptConfig) -> Self {
        self.prompts = prompts;
        self
    }

    /// 为 "Planning" (规划) 阶段指定特定 LLM。
    /// 建议：使用推理能力最强的模型 (如 GPT-4, Claude 3.5 Sonnet)。
    pub fn with_planning_llm(mut self, llm: Arc<dyn ModelBackend>) -> Self {
        self.specific_llms.insert("planning".to_string(), llm);
        self
    }

    /// 为 "Review" (审查) 阶段指定特定 LLM。
    /// 用于检查规划的合理性。
    pub fn with_review_llm(mut self, llm: Arc<dyn ModelBackend>) -> Self {
        self.specific_llms.insert("review".to_string(), llm);
        self
    }

    /// 为 "Verification" (验证) 阶段指定特定 LLM。
    /// 建议：使用速度快且便宜的模型 (如 GPT-3.5, Haiku)，以便支持多次投票验证。
    pub fn with_verification_llm(mut self, llm: Arc<dyn ModelBackend>) -> Self {
        self.specific_llms.insert("verification".to_string(), llm);
        self
    }

    /// 构建最终的 ManagerAgent 实例
    pub fn build(self) -> ManagerAgent {
        // 1. 基于默认模型初始化配置
        let mut llm_config = AgentLLMConfig::new(self.default_llm.clone());

        // 2. 如果指定了特定阶段的模型，则进行覆盖
        if let Some(l) = self.specific_llms.get("planning") {
            llm_config.planning = l.clone();
        }
        if let Some(l) = self.specific_llms.get("review") {
            llm_config.review = l.clone();
        }
        if let Some(l) = self.specific_llms.get("verification") {
            llm_config.verification = l.clone();
        }
        // ... 其他阶段如 adjudication, reflection, synthesis 若有特定设置，在此处扩展 ...

        ManagerAgent {
            name: self.name,
            skills: HashMap::new(),
            llms: llm_config, // 使用组装好的 LLM 配置
            store: self.store,
            prompts: self.prompts, // 使用配置好的 Prompt 模板
        }
    }
}

// --- 数据结构 ---

/// 子任务定义，由规划阶段生成
#[derive(Debug, Deserialize, Serialize, Clone)]
struct SubTask {
    /// 任务唯一标识符
    id: String,
    /// 任务的具体指令描述
    description: String,
    /// 依赖的任务 ID 列表 (用于构建 DAG)
    #[serde(default)]
    dependencies: Vec<String>,
    /// 指定执行该任务所需的技能名称
    skill_name: String,
    /// 传递给技能的参数 (可能包含模板变量 {{task_id.field}})
    #[serde(default)]
    params: Value,
    /// 任务完成的验收标准 (用于验证阶段)
    #[serde(default)]
    acceptance_criteria: String,
}

/// 执行计划，包含思维链 (Thought) 和任务列表
#[derive(Debug, Deserialize)]
struct ExecutionPlan {
    #[allow(dead_code)]
    thought: String,
    tasks: Vec<SubTask>,
}

/// 任务执行结果
#[derive(Debug)]
struct TaskResult {
    task_id: String,
    success: bool,
    summary: String,
    /// 任务产生的结构化数据 (Artifacts)
    output_data: Option<Value>,
    /// 如果失败或验证未通过，此处包含反馈信息
    verification_feedback: Option<String>,
}

/// 验证阶段的单次投票结果
#[derive(Debug, Deserialize)]
struct VerificationResult {
    passed: bool,
    reason: String,
    suggestion: String,
}

/// 仲裁阶段的结果 (当验证出现分歧时使用)
#[derive(Debug, Deserialize)]
struct AdjudicationResult {
    final_decision: bool,
    rationale: String,
}

/// **Manager Agent**：全能型任务编排代理。
///
/// 核心职责：
/// 1. 解析用户指令并规划任务 (Planning)。
/// 2. 审查并优化计划 (Review)。
/// 3. 并行执行任务，管理依赖关系 (Execution & DAG)。
/// 4. 验证任务结果，必要时进行反思重试 (Verification & Reflection)。
/// 5. 综合所有结果生成最终输出 (Synthesis)。
pub struct ManagerAgent {
    name: String,
    /// 注册的工具/技能集合
    skills: HashMap<String, Arc<dyn AgentSkill>>,

    /// LLM 注册表，包含不同阶段 (Planning, Review, etc.) 的专用模型
    llms: AgentLLMConfig,

    /// 上下文存储 (Memory/History)
    store: Arc<dyn ContextStore>,

    /// Prompt 配置集，定义了各个阶段的系统提示词
    prompts: PromptConfig,
}

impl ManagerAgent {
    /// 快捷构造函数：使用默认 Prompt 和单一 LLM 处理所有阶段
    pub fn new(
        name: &str,
        default_llm: Arc<dyn ModelBackend>,
        store: Arc<dyn ContextStore>,
    ) -> Self {
        ManagerAgentBuilder::new(name, default_llm, store).build()
    }

    /// 获取 Builder 以进行高级定制（自定义 Prompt 或 多 LLM 配置）
    pub fn builder(
        name: &str,
        default_llm: Arc<dyn ModelBackend>,
        store: Arc<dyn ContextStore>,
    ) -> ManagerAgentBuilder {
        ManagerAgentBuilder::new(name, default_llm, store)
    }

    /// 注册一项技能 (AgentSkill)
    pub fn register_skill(&mut self, skill: impl AgentSkill + 'static) {
        self.skills
            .insert(skill.name().to_string(), Arc::new(skill));
    }

    /// **核心入口**：执行一个高层指令。
    ///
    /// 流程：
    /// 1. Planning: 生成初始计划。
    /// 2. Review: 优化计划。
    /// 3. Loop: 执行 DAG (有向无环图) 调度。
    ///    - 找出当前无依赖的可执行任务。
    ///    - 并行执行。
    ///    - 如果失败，进行全局重规划 (Global Replanning)。
    /// 4. Synthesis: 汇总结果。
    pub async fn run_task<T>(&self, instruction: &str, output_schema_desc: &str) -> Result<T>
    where
        T: DeserializeOwned + Send,
    {
        // 初始化运行上下文
        let ctx = Arc::new(Mutex::new(AgentContext::new()));
        {
            let mut c = ctx.lock().await;
            c.history.push(format!("User Instruction: {}", instruction));
        }

        tracing::info!("[{}] Analysis & Planning task: {}", self.name, instruction);

        // --- Phase 1: Planning (规划) ---
        // 使用 Planning LLM 根据已注册的 Skills 生成任务列表
        let initial_plan = self.make_plan(instruction).await?;

        // --- Phase 2: Review (审查) ---
        // 使用 Review LLM 检查并优化计划
        let mut current_plan_tasks = self
            .review_and_refine_plan(instruction, initial_plan)
            .await?;

        tracing::info!(
            "[{}] Optimized Plan: {} tasks",
            self.name,
            current_plan_tasks.len()
        );

        let mut completed_tasks_ids = HashSet::new();
        let mut task_results_history: Vec<(String, String)> = Vec::new(); // (id, summary)
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

            // 3.4 错误处理与全局重规划 (Global Replanning)
            if batch_failed {
                if global_replan_count >= MAX_GLOBAL_REPLANS {
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

                // 调用 Replanning LLM，根据失败原因调整剩余任务
                let new_tasks = self
                    .replan_remaining_tasks(
                        instruction,
                        &current_plan_tasks,
                        &completed_tasks_ids,
                        &task_results_history,
                        &failure_info,
                    )
                    .await?;

                // 合并：保留已完成的任务，替换未完成的任务
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

        // 使用 Synthesis LLM 将所有过程数据格式化为用户请求的最终类型 T
        let final_result = self
            .synthesize_final_result::<T>(
                instruction,
                &history_text,
                &artifacts_json,
                output_schema_desc,
            )
            .await?;

        // 持久化上下文
        self.persist_context(&*ctx.lock().await).await;

        Ok(final_result)
    }

    // --- Pipeline Implementation ---

    /// 并行执行一批任务。
    /// 为每个任务克隆必要的配置和上下文引用。
    async fn execute_batch_parallel(
        &self,
        tasks: Vec<SubTask>,
        ctx: Arc<Mutex<AgentContext>>,
    ) -> Vec<Result<TaskResult>> {
        // 获取当前的 Artifacts 快照，用于解析参数依赖
        let artifacts_snapshot = { ctx.lock().await.artifacts.clone() };
        let mut futures = Vec::new();

        for task in tasks {
            let skill_map = self.skills.clone();
            // 传递 LLM 配置和 Prompt 配置
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

    /// 单个任务的完整执行流水线：
    /// Prepare Params -> Execute Skill -> Verify (Vote + Adjudicate) -> Reflect & Retry
    #[allow(clippy::too_many_arguments)]
    async fn execute_task_pipeline(
        task: SubTask,
        skills: HashMap<String, Arc<dyn AgentSkill>>,
        ctx: Arc<Mutex<AgentContext>>,
        artifacts: HashMap<String, Value>,
        llms: AgentLLMConfig,
        prompts: PromptConfig,
    ) -> Result<TaskResult> {
        // 1. 参数解析：处理 {{task_id.field}} 形式的依赖引用
        let resolved_params = Self::resolve_params_deep(&task.params, &artifacts);

        let max_retries = 2;
        let mut current_skill_name = task.skill_name.clone();
        let mut current_params = resolved_params;
        let mut last_failure_reason = String::new();

        // 2. 重试循环 (Retry Loop)
        for attempt in 0..=max_retries {
            if let Some(skill) = skills.get(&current_skill_name) {
                tracing::info!(
                    "  -> Executing [{}] (Attempt {}/{})",
                    task.id,
                    attempt + 1,
                    max_retries + 1
                );

                let payload = TaskPayload {
                    instruction: task.description.clone(),
                    params: current_params.clone(),
                };

                // 执行具体的 Skill
                let execution_result = {
                    let mut c = ctx.lock().await;
                    skill.execute(&mut c, payload).await
                };

                match execution_result {
                    Ok(result) => {
                        // 3. 验证阶段 (Verification Stage)
                        // 结合了多重投票 (Verification LLM) 和 仲裁 (Adjudication LLM)
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
                            tracing::warn!(
                                "Task [{}] Verification Rejected: {}",
                                task.id,
                                verify_msg
                            );
                            last_failure_reason = verify_msg;
                        }
                    }
                    Err(e) => {
                        last_failure_reason = format!("Runtime Error: {}", e);
                        tracing::warn!("Task [{}] Runtime Error: {}", task.id, last_failure_reason);
                    }
                }

                // 4. 反思阶段 (Reflection Stage)
                // 如果任务失败且仍有重试次数，调用 Reflection LLM 分析原因并建议修正（修改参数或更换技能）
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
                        Ok((new_skill, new_params, reason)) => {
                            tracing::info!(
                                "Reflection -> Retry with '{}'. Reason: {}",
                                new_skill,
                                reason
                            );
                            current_skill_name = new_skill;
                            current_params = new_params;
                        }
                        Err(e) => tracing::error!("Reflection logic failed: {}", e),
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

        // 所有重试均失败
        Ok(TaskResult {
            task_id: task.id,
            success: false,
            summary: format!("Retries exhausted. Last error: {}", last_failure_reason),
            output_data: None,
            verification_feedback: Some(last_failure_reason),
        })
    }

    /// 高级验证逻辑：投票 + 仲裁
    /// 1. 并行调用 3 次 Verification LLM 进行投票。
    /// 2. 如果一致通过，则成功；一致拒绝，则失败。
    /// 3. 如果存在分歧 (Conflict)，调用 Adjudication LLM 进行最终裁决。
    async fn verify_with_adjudication(
        llms: &AgentLLMConfig,
        verify_tmpl: &str,
        adjudicate_tmpl: &str,
        task: &SubTask,
        output: &str,
    ) -> Result<(bool, String)> {
        // 1. 发起 3 次投票 (使用 Verification LLM)
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

        let mut pass_results = Vec::new();
        let mut fail_results = Vec::new();

        for res in results {
            match res {
                Ok(v) => {
                    if v.passed {
                        pass_results.push(v);
                    } else {
                        fail_results.push(v);
                    }
                }
                Err(_) => {}
            }
        }

        let pass_count = pass_results.len();
        let fail_count = fail_results.len();

        // 一致性检查
        if fail_count == 0 {
            return Ok((true, "Unanimous Pass".into()));
        }
        if pass_count == 0 {
            let reasons = fail_results
                .iter()
                .map(|r| r.reason.as_str())
                .collect::<Vec<_>>()
                .join(" | ");
            return Ok((false, format!("Unanimous Fail: {}", reasons)));
        }

        tracing::warn!(
            "Task [{}] Conflict ({} vs {}). Summoning Adjudicator.",
            task.id,
            pass_count,
            fail_count
        );

        // 2. 准备仲裁所需的上下文
        let pass_reasons = pass_results
            .iter()
            .map(|r| format!("- [Pass]: {}", r.reason))
            .collect::<Vec<_>>()
            .join("\n");
        let fail_reasons = fail_results
            .iter()
            .map(|r| format!("- [Reject]: {}", r.reason))
            .collect::<Vec<_>>()
            .join("\n");
        let conflict_summary = format!(
            "Arguments for PASS:\n{}\n\nArguments for REJECT:\n{}",
            pass_reasons, fail_reasons
        );

        let prompt = adjudicate_tmpl
            .replace("{{task_description}}", &task.description)
            .replace("{{acceptance_criteria}}", &task.acceptance_criteria)
            .replace("{{actual_output}}", output)
            .replace("{{verification_conflict}}", &conflict_summary);

        // 3. 核心：使用 Adjudication LLM 进行裁决
        let raw = llms.adjudication.chat(&prompt, "Adjudicate").await?;
        let json = Self::clean_json_markdown(&raw);
        let adj_res: AdjudicationResult = serde_json::from_str(&json)?;

        Ok((adj_res.final_decision, adj_res.rationale))
    }

    /// 单次验证请求
    async fn verify_output_single(
        llm: &Arc<dyn ModelBackend>,
        template: &str,
        task: &SubTask,
        output: &str,
    ) -> Result<VerificationResult> {
        let prompt = template
            .replace("{{task_description}}", &task.description)
            .replace("{{acceptance_criteria}}", &task.acceptance_criteria)
            .replace("{{actual_output}}", output);

        let raw = llm.chat(&prompt, "Verify Output").await?;
        let json = Self::clean_json_markdown(&raw);
        Ok(serde_json::from_str(&json)?)
    }

    // --- Helpers (LLM Specific) ---

    /// 阶段：生成初始计划
    async fn make_plan(&self, instruction: &str) -> Result<ExecutionPlan> {
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

        // 使用 Planning LLM
        let raw = self.llms.planning.chat(&prompt, "Initial Plan").await?;
        let json = Self::clean_json_markdown(&raw);
        Ok(serde_json::from_str(&json)?)
    }

    /// 阶段：审查并优化计划
    async fn review_and_refine_plan(
        &self,
        instruction: &str,
        plan: ExecutionPlan,
    ) -> Result<Vec<SubTask>> {
        let plan_json = serde_json::to_string_pretty(&plan.tasks)?;
        let skill_desc_str = self.skills.keys().cloned().collect::<Vec<_>>().join(", ");
        let prompt = self
            .prompts
            .plan_review_prompt
            .replace("{{user_instruction}}", instruction)
            .replace("{{current_plan}}", &plan_json)
            .replace("{{available_skills}}", &skill_desc_str);

        // 使用 Review LLM
        let raw = self.llms.review.chat(&prompt, "Review Plan").await?;
        let json = Self::clean_json_markdown(&raw);
        let new_plan: ExecutionPlan = serde_json::from_str(&json).unwrap_or(plan);
        Ok(new_plan.tasks)
    }

    /// 阶段：重规划 (当部分任务失败且无法本地恢复时)
    async fn replan_remaining_tasks(
        &self,
        instruction: &str,
        plan: &[SubTask],
        done: &HashSet<String>,
        results: &[(String, String)],
        reason: &str,
    ) -> Result<Vec<SubTask>> {
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

    /// 阶段：反思 (Reflection)
    /// 当任务执行失败或验证不通过时，分析原因并建议新的参数或技能。
    async fn reflect_and_reroute(
        llm: &Arc<dyn ModelBackend>,
        tmpl: &str,
        task: &SubTask,
        err: &str,
        skills: &HashMap<String, Arc<dyn AgentSkill>>,
    ) -> Result<(String, Value, String)> {
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

    /// 阶段：最终综合 (Synthesis)
    async fn synthesize_final_result<T: DeserializeOwned>(
        &self,
        instruction: &str,
        history: &str,
        artifacts_json: &str,
        schema: &str,
    ) -> Result<T> {
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

    // --- Helpers (DAG & Data) ---

    /// 查找当前所有前置依赖 (Dependencies) 均已满足的任务。
    /// 包含死锁检测逻辑。
    fn find_executable_tasks(
        &self,
        all: &[SubTask],
        done: &HashSet<String>,
    ) -> Result<Vec<SubTask>> {
        let executable: Vec<SubTask> = all
            .iter()
            .filter(|t| !done.contains(&t.id)) // 过滤掉已完成的
            .filter(|t| t.dependencies.iter().all(|d| done.contains(d))) // 检查依赖是否都在 done 中
            .cloned()
            .collect();

        // 如果没有可执行的任务，但仍有未完成的任务，说明存在循环依赖或逻辑死锁
        if executable.is_empty() && all.iter().any(|t| !done.contains(&t.id)) {
            return Err(anyhow::anyhow!("Plan Deadlock detected."));
        }
        Ok(executable)
    }

    /// 递归解析参数中的模板变量。
    /// 支持将字符串中的 `{{taskId.field}}` 替换为上下文中 `artifacts` 的实际值。
    fn resolve_params_deep(params: &Value, artifacts: &HashMap<String, Value>) -> Value {
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
                        let mut current_val = root_val;
                        let mut found = true;
                        // 遍历路径 (e.g., artifacts["task1"]["data"]["result"])
                        for &field in &parts[1..] {
                            match current_val {
                                Value::Object(map) => {
                                    if let Some(v) = map.get(field) {
                                        current_val = v;
                                    } else {
                                        found = false;
                                        break;
                                    }
                                }
                                Value::Array(arr) => {
                                    if let Ok(idx) = field.parse::<usize>() {
                                        if let Some(v) = arr.get(idx) {
                                            current_val = v;
                                        } else {
                                            found = false;
                                            break;
                                        }
                                    } else {
                                        found = false;
                                        break;
                                    }
                                }
                                _ => {
                                    found = false;
                                    break;
                                }
                            }
                        }
                        if found {
                            return current_val.clone();
                        }
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

    /// 处理一批任务的执行结果。
    /// 这里的处理是同步更新状态，决定是否需要全局重规划。
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
                // 将 output_data 存入 artifacts 供后续任务引用
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

    /// 清理 LLM 返回的 Markdown 代码块格式
    fn clean_json_markdown(input: &str) -> String {
        input
            .trim()
            .strip_prefix("```json")
            .unwrap_or(input)
            .strip_prefix("```")
            .unwrap_or(input)
            .strip_suffix("```")
            .unwrap_or(input)
            .trim()
            .to_string()
    }
}
