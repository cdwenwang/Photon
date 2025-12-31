use anyhow::{Context, Result};
use futures::future::join_all;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::llm::ModelBackend;
use crate::skills::AgentSkill;
use crate::store::ContextStore;
use crate::types::{AgentContext, TaskPayload};

// --- 数据结构 ---

/// 单个子任务的定义
#[derive(Debug, Deserialize, Serialize, Clone)]
struct SubTask {
    /// 任务唯一标识
    id: String,
    /// 任务描述
    description: String,
    /// 依赖的任务ID列表，DAG 调度的核心依据
    #[serde(default)]
    dependencies: Vec<String>,
    /// 指定执行该任务的 Skill 名称
    skill_name: String,
    /// 执行参数
    #[serde(default)]
    params: Value,
}

/// 整体执行规划
#[derive(Debug, Deserialize)]
struct ExecutionPlan {
    #[allow(dead_code)]
    thought: String,
    tasks: Vec<SubTask>,
}

/// 任务执行结果封装
#[derive(Debug)]
struct TaskResult {
    task_id: String,
    success: bool,
    summary: String,
    #[allow(dead_code)]
    output_data: Option<Value>,
}

/// Manager Agent：负责任务规划、调度、执行和结果综合
///
/// 该结构体作为 Agent 的核心控制器，维护了 LLM 后端、能力集 (Skills)、
/// 上下文存储以及用于指导 LLM 行为的各类 Prompt 模板。
pub struct ManagerAgent {
    /// Agent 的唯一名称或标识符。
    /// 主要用于日志追踪 (Tracing) 和区分不同的 Agent 实例。
    name: String,

    /// 能力注册表 (Skill Registry)。
    /// 存储了该 Agent 可调用的所有工具/技能。
    /// - Key: 技能名称 (Skill Name)
    /// - Value: 线程安全的技能实现 (Arc<dyn AgentSkill>)
    skills: HashMap<String, Arc<dyn AgentSkill>>,

    /// LLM 模型后端接口。
    /// 负责处理所有的自然语言生成请求（包括规划、反思、重规划、总结）。
    /// 使用 `Arc` 以支持在多线程任务中共享同一个模型实例。
    llm: Arc<dyn ModelBackend>,

    /// 上下文存储接口。
    /// 用于持久化保存对话历史 (History) 和执行上下文，确保 Agent 状态不丢失。
    store: Arc<dyn ContextStore>,

    // --- Prompt 模板配置 (Prompt Templates) ---
    /// **规划 (Planning) 模板**。
    ///
    /// 用于在任务开始时，将用户的高层指令拆解为结构化的任务有向无环图 (DAG)。
    /// 模板中通常需要包含 `{{user_instruction}}` (用户指令) 和 `{{skill_descriptions}}` (可用工具列表)。
    planning_prompt_template: String,

    /// **局部反思 (Local Reflection) 模板**。
    ///
    /// 当单个 AgentSkill 执行失败（且未达到最大重试次数）时调用。
    /// 用于引导 LLM 分析具体的错误信息，并建议更换参数或更换备用 Skill。
    /// 模板中通常需要包含 `{{error_msg}}`, `{{failed_skill}}` 等占位符。
    reflection_prompt_template: String,

    /// **全局重规划 (Global Replanning) 模板**。
    ///
    /// 当某个任务节点彻底失败，导致后续依赖无法满足时调用。
    /// 用于引导 LLM 基于“已完成的任务”和“失败原因”，重新生成剩余的任务路径。
    /// 模板中通常需要包含 `{{completed_desc}}`, `{{failure_reason}}` 等占位符。
    replanning_prompt_template: String,

    /// **结果综合 (Synthesis) 模板**。
    ///
    /// 在所有任务流程结束后调用。
    /// 用于将零散的执行历史 (Execution History) 汇总并转换为符合业务要求的泛型结构 (JSON)。
    /// 模板中必须包含 `{{history}}` (执行记录) 和 `{{schema}}` (目标输出格式描述)。
    synthesis_prompt_template: String,
}

impl ManagerAgent {
    pub fn new(
        name: &str,
        llm: Arc<dyn ModelBackend>,
        store: Arc<dyn ContextStore>,
        planning_prompt_template: String,
        reflection_prompt_template: String,
        replanning_prompt_template: String,
        synthesis_prompt_template: String,
    ) -> Self {
        Self {
            name: name.to_string(),
            skills: HashMap::new(),
            llm,
            store,
            planning_prompt_template,
            reflection_prompt_template,
            replanning_prompt_template,
            synthesis_prompt_template,
        }
    }

    pub fn register_skill(&mut self, skill: impl AgentSkill + 'static) {
        self.skills
            .insert(skill.name().to_string(), Arc::new(skill));
    }

    /// 核心入口：执行任务并返回泛型结果 T
    ///
    /// 该方法包含完整的 Agent 自治流程：
    /// 1. 任务规划 (Planning)
    /// 2. DAG 动态调度与并行执行 (Scheduling & Execution)
    /// 3. 错误恢复与重规划 (Replanning)
    /// 4. 结果综合 (Synthesis)
    ///
    /// # 参数
    /// * `instruction`: 用户指令
    /// * `output_schema_desc`: 对 T 的自然语言描述或 JSON Schema，用于指导 LLM 生成格式
    pub async fn run_task<T>(&self, instruction: &str, output_schema_desc: &str) -> Result<T>
    where
        T: DeserializeOwned + Send,
    {
        // 1. 初始化上下文
        let ctx = Arc::new(Mutex::new(AgentContext::new()));
        {
            let mut c = ctx.lock().await;
            c.history.push(format!("User Instruction: {}", instruction));
        }

        tracing::info!("[{}] Analysis & Planning task: {}", self.name, instruction);

        // 2. 初始规划
        let mut current_plan_tasks = self.make_plan(instruction).await?.tasks;
        tracing::info!(
            "[{}] Initial Plan: {} tasks",
            self.name,
            current_plan_tasks.len()
        );

        // 3. 执行状态维护
        let mut completed_tasks_ids = HashSet::new();
        let mut task_results_history: Vec<(String, String)> = Vec::new(); // (id, summary)
        let mut global_replan_count = 0;
        const MAX_GLOBAL_REPLANS: usize = 3;

        // 4. 主调度循环 (DAG Execution Loop)
        loop {
            // 4.1 检查完成状态
            if current_plan_tasks
                .iter()
                .all(|t| completed_tasks_ids.contains(&t.id))
            {
                break;
            }

            // 4.2 获取当前可执行的任务批次
            let executable_tasks =
                self.find_executable_tasks(&current_plan_tasks, &completed_tasks_ids)?;

            tracing::info!(
                "[{}] Batch executing: {:?}",
                self.name,
                executable_tasks.iter().map(|t| &t.id).collect::<Vec<_>>()
            );

            // 4.3 并行执行批次任务
            let results = self
                .execute_batch_parallel(executable_tasks, ctx.clone())
                .await;

            // 4.4 处理执行结果
            let (batch_failed, failure_info) = self
                .process_execution_results(
                    results,
                    &mut completed_tasks_ids,
                    &mut task_results_history,
                    ctx.clone(),
                )
                .await?;

            // 4.5 故障恢复：全局重规划
            if batch_failed {
                if global_replan_count >= MAX_GLOBAL_REPLANS {
                    return Err(anyhow::anyhow!(
                        "Exceeded max global replanning limit. Last failure: {}",
                        failure_info
                    ));
                }

                global_replan_count += 1;
                tracing::warn!(
                    "[{}] Global Replanning ({}/{}). Reason: {}",
                    self.name,
                    global_replan_count,
                    MAX_GLOBAL_REPLANS,
                    failure_info
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

                // 合并计划：保留已完成任务 + 追加新任务
                let mut valid_tasks: Vec<SubTask> = current_plan_tasks
                    .into_iter()
                    .filter(|t| completed_tasks_ids.contains(&t.id))
                    .collect();
                valid_tasks.extend(new_tasks);
                current_plan_tasks = valid_tasks;

                tracing::info!(
                    "[{}] Plan Updated. Tasks: {}",
                    self.name,
                    current_plan_tasks.len()
                );
            }
        }

        // 5. 结果综合与持久化
        tracing::info!("[{}] Synthesizing final result...", self.name);

        // 获取只读历史副本进行总结
        let history_text = ctx.lock().await.history.join("\n");
        let final_result = self
            .synthesize_final_result::<T>(instruction, &history_text, output_schema_desc)
            .await?;

        let final_context = ctx.lock().await;
        self.persist_context(&final_context).await;

        Ok(final_result)
    }

    // --- 内部逻辑拆解方法 ---

    /// 根据 DAG 依赖关系，筛选当前可执行的任务
    fn find_executable_tasks(
        &self,
        all_tasks: &[SubTask],
        completed_ids: &HashSet<String>,
    ) -> Result<Vec<SubTask>> {
        let executable_tasks: Vec<SubTask> = all_tasks
            .iter()
            .filter(|t| !completed_ids.contains(&t.id)) // 排除已完成的
            .filter(|t| {
                // 检查所有依赖是否都已完成
                t.dependencies.iter().all(|dep| completed_ids.contains(dep))
            })
            .cloned()
            .collect();

        // 死锁检测：如果有未完成的任务，但没有可执行的任务，说明存在循环依赖或逻辑断层
        if executable_tasks.is_empty() {
            let pending_ids: Vec<_> = all_tasks
                .iter()
                .filter(|t| !completed_ids.contains(&t.id))
                .map(|t| &t.id)
                .collect();

            if !pending_ids.is_empty() {
                return Err(anyhow::anyhow!(
                    "Plan deadlock: dependencies cannot be resolved. Pending tasks: {:?}",
                    pending_ids
                ));
            }
        }

        Ok(executable_tasks)
    }

    /// 并行执行一批任务
    async fn execute_batch_parallel(
        &self,
        tasks: Vec<SubTask>,
        ctx: Arc<Mutex<AgentContext>>,
    ) -> Vec<Result<TaskResult>> {
        let mut futures = Vec::new();

        for task in tasks {
            // 克隆必要的资源以移动到 Future 中
            let skill_map = self.skills.clone();
            let llm = self.llm.clone();
            // 获取上下文快照，避免长时间持有锁，且支持并行读取
            let context_snapshot = { ctx.lock().await.clone() };
            let reflection_prompt = self.reflection_prompt_template.clone();

            futures.push(async move {
                Self::execute_task_with_retry(
                    task,
                    skill_map,
                    context_snapshot,
                    llm,
                    reflection_prompt,
                )
                .await
            });
        }

        join_all(futures).await
    }

    /// 处理批次执行结果，更新状态，并检测是否需要触发重规划
    /// 返回: (batch_failed: bool, failure_info: String)
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
            let res = res?; // 传播 Panic 或系统级错误

            let mut c = ctx.lock().await;
            c.history.push(format!(
                "Task [{}] finished. Success: {}",
                res.task_id, res.success
            ));

            if res.success {
                completed_ids.insert(res.task_id.clone());
                history_log.push((res.task_id.clone(), res.summary.clone()));
                c.history.push(format!("Output: {}", res.summary));
            } else {
                batch_failed = true;
                failure_info = format!(
                    "Task '{}' failed after retries. Error: {}",
                    res.task_id, res.summary
                );
                c.history
                    .push(format!("CRITICAL FAILURE: {}", failure_info));
            }
        }

        Ok((batch_failed, failure_info))
    }

    /// 最终结果综合：调用 LLM 将执行历史转换为结构化数据
    async fn synthesize_final_result<T>(
        &self,
        instruction: &str,
        history_text: &str,
        schema_desc: &str,
    ) -> Result<T>
    where
        T: DeserializeOwned,
    {
        let synthesis_prompt = self
            .synthesis_prompt_template
            .replace("{{instruction}}", instruction)
            .replace("{{history}}", history_text)
            .replace("{{schema}}", schema_desc);

        let raw_response = self
            .llm
            .chat(&synthesis_prompt, "Synthesize Final Output")
            .await?;

        let json_str = Self::clean_json_markdown(&raw_response);

        let result: T = serde_json::from_str(&json_str).context(format!(
            "Failed to parse synthesis result. Raw: {}",
            json_str
        ))?;

        Ok(result)
    }

    // --- 以下是原有的辅助方法 (保持不变或微调注释) ---

    /// 执行单个任务，包含【局部反思与重试】机制
    async fn execute_task_with_retry(
        task: SubTask,
        skills: HashMap<String, Arc<dyn AgentSkill>>,
        mut ctx: AgentContext,
        llm: Arc<dyn ModelBackend>,
        reflection_template: String,
    ) -> Result<TaskResult> {
        let max_retries = 2;
        let mut current_skill_name = task.skill_name.clone();
        let mut current_params = task.params.clone();

        for attempt in 0..=max_retries {
            if let Some(skill) = skills.get(&current_skill_name) {
                let payload = TaskPayload {
                    instruction: task.description.clone(),
                    params: current_params.clone(),
                };

                tracing::info!(
                    "  -> Executing [{}] (Attempt {}/{})",
                    task.id,
                    attempt + 1,
                    max_retries + 1
                );

                match skill.execute(&mut ctx, payload).await {
                    Ok(result) => {
                        return Ok(TaskResult {
                            task_id: task.id,
                            success: true,
                            summary: result.summary,
                            output_data: result.data,
                        });
                    }
                    Err(e) => {
                        if attempt < max_retries {
                            tracing::warn!("Task [{}] failed: {}. Reflecting...", task.id, e);

                            // 局部反思：尝试调整参数或更换 Skill
                            match Self::reflect_and_reroute(
                                &llm,
                                &reflection_template,
                                &task,
                                &e.to_string(),
                                &skills,
                            )
                            .await
                            {
                                Ok((new_skill, new_params, reason)) => {
                                    tracing::info!(
                                        "Reflection: Switch to '{}'. Reason: {}",
                                        new_skill,
                                        reason
                                    );
                                    current_skill_name = new_skill;
                                    current_params = new_params;
                                }
                                Err(re_err) => {
                                    tracing::error!("Reflection failed: {}", re_err);
                                }
                            }
                        } else {
                            return Ok(TaskResult {
                                task_id: task.id,
                                success: false,
                                summary: format!("Local retries exhausted. Last error: {}", e),
                                output_data: None,
                            });
                        }
                    }
                }
            } else {
                return Ok(TaskResult {
                    task_id: task.id,
                    success: false,
                    summary: format!("Skill '{}' not found.", current_skill_name),
                    output_data: None,
                });
            }
        }

        Ok(TaskResult {
            task_id: task.id,
            success: false,
            summary: "Unknown error flow".to_string(),
            output_data: None,
        })
    }

    /// 局部反思：请求 LLM 建议新的 Skill 或参数
    async fn reflect_and_reroute(
        llm: &Arc<dyn ModelBackend>,
        template: &str,
        task: &SubTask,
        error_msg: &str,
        skills: &HashMap<String, Arc<dyn AgentSkill>>,
    ) -> Result<(String, Value, String)> {
        let skill_list = skills.keys().cloned().collect::<Vec<_>>().join(", ");

        let prompt = template
            .replace("{{task_description}}", &task.description)
            .replace("{{failed_skill}}", &task.skill_name)
            .replace("{{current_params}}", &task.params.to_string())
            .replace("{{error_msg}}", error_msg)
            .replace("{{available_skills}}", &skill_list);

        let raw = llm.chat(&prompt, "Fix execution").await?;
        let json_str = Self::clean_json_markdown(&raw);

        #[derive(Deserialize)]
        struct ReflectionDecision {
            new_skill: String,
            new_params: Value,
            reason: String,
        }

        let decision: ReflectionDecision =
            serde_json::from_str(&json_str).context("Failed to parse reflection JSON")?;

        Ok((decision.new_skill, decision.new_params, decision.reason))
    }

    /// 生成初始任务计划
    async fn make_plan(&self, instruction: &str) -> Result<ExecutionPlan> {
        let skill_desc_str = self
            .skills
            .values()
            .map(|s| format!("- **{}**: {}", s.name(), s.description()))
            .collect::<Vec<_>>()
            .join("\n");

        let prompt = self
            .planning_prompt_template
            .replace("{{skill_descriptions}}", &skill_desc_str)
            .replace("{{user_instruction}}", instruction);

        let raw = self.llm.chat(&prompt, instruction).await?;
        let json = Self::clean_json_markdown(&raw);
        Ok(serde_json::from_str(&json)?)
    }

    /// 全局重规划：基于已完成任务和失败原因生成新路径
    async fn replan_remaining_tasks(
        &self,
        original_instruction: &str,
        current_plan: &[SubTask],
        completed_ids: &HashSet<String>,
        completed_results: &[(String, String)],
        failure_reason: &str,
    ) -> Result<Vec<SubTask>> {
        let completed_desc = completed_results
            .iter()
            .map(|(id, output)| format!("- Task [{}]: DONE. Output: {}", id, output))
            .collect::<Vec<_>>()
            .join("\n");

        let pending_desc = current_plan
            .iter()
            .filter(|t| !completed_ids.contains(&t.id))
            .map(|t| format!("- Task [{}]: {}", t.id, t.description))
            .collect::<Vec<_>>()
            .join("\n");

        let skill_desc = self
            .skills
            .values()
            .map(|s| format!("- {}", s.name()))
            .collect::<Vec<_>>()
            .join(", ");

        let prompt = self
            .replanning_prompt_template
            .replace("{{goal}}", original_instruction)
            .replace("{{completed_desc}}", &completed_desc)
            .replace("{{failure_reason}}", failure_reason)
            .replace("{{pending_desc}}", &pending_desc)
            .replace("{{skills}}", &skill_desc);

        let raw = self.llm.chat(&prompt, "Replan graph").await?;
        let json_str = Self::clean_json_markdown(&raw);

        let new_plan: ExecutionPlan =
            serde_json::from_str(&json_str).context("Failed to parse replanning JSON")?;

        Ok(new_plan.tasks)
    }

    fn clean_json_markdown(input: &str) -> String {
        let input = input.trim();
        if input.starts_with("```json") {
            input
                .strip_prefix("```json")
                .unwrap_or(input)
                .strip_suffix("```")
                .unwrap_or(input)
                .trim()
                .to_string()
        } else if input.starts_with("```") {
            input
                .strip_prefix("```")
                .unwrap_or(input)
                .strip_suffix("```")
                .unwrap_or(input)
                .trim()
                .to_string()
        } else {
            input.to_string()
        }
    }

    async fn persist_context(&self, ctx: &AgentContext) {
        if let Err(e) = self.store.save(ctx).await {
            tracing::error!("Failed to save context: {}", e);
        }
    }
}
