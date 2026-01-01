use anyhow::Result;
use async_trait::async_trait;
use quant_agent::llm::gemini3_flash;
use quant_agent::manager::ManagerAgent;
use quant_agent::skills::AgentSkill;
use quant_agent::store::local;
use quant_agent::types::{AgentContext, TaskPayload, TaskResult};
use serde::Deserialize;
use serde_json::json;
use std::path::PathBuf;
use std::sync::Arc;
/// 3. 模拟技能 (Mock Skill)
/// 一个简单的加法技能
struct AddSkill;

#[async_trait]
impl AgentSkill for AddSkill {
    fn name(&self) -> &str {
        "add_numbers"
    }

    fn description(&self) -> &str {
        "Adds two numbers. Params: {'a': number, 'b': number}"
    }

    async fn execute(&self, _ctx: &mut AgentContext, payload: TaskPayload) -> Result<TaskResult> {
        let a = payload.params["a"].as_f64().unwrap_or(0.0);
        let b = payload.params["b"].as_f64().unwrap_or(0.0);
        let sum = a + b;

        Ok(TaskResult {
            summary: format!("Calculated {} + {} = {}", a, b, sum),
            data: Some(json!({ "result": sum })),
        })
    }
}

// --- Test Cases ---

#[tokio::test]
async fn test_manager_happy_path() -> Result<()> {
    // === 场景描述 ===
    // 用户指令: "Calculate 10 + 20"
    // 预期流程:
    // 1. Planning: 生成一个调用 add_numbers 的计划
    // 2. Review: 确认计划
    // 3. Execution: 执行 AddSkill
    // 4. Verification: 投票通过
    // 5. Synthesis: 输出最终结果

    // --- 1. 构造 Mock LLM 的响应数据 ---

    // (Planning 阶段的响应)
    let plan_json = json!({
        "thought": "I need to calculate the sum.",
        "tasks": [
            {
                "id": "task_1",
                "description": "Calculate sum of 10 and 20",
                "skill_name": "add_numbers",
                "dependencies": [],
                "params": { "a": 10, "b": 20 },
                "acceptance_criteria": "Result should be 30"
            }
        ]
    })
    .to_string();

    // (Review 阶段的响应 - 通常会返回同样的 Plan 结构)
    let review_json = plan_json.clone();

    // (Verification 阶段的响应 - 模拟3次投票，全部通过)
    let verification_pass = json!({
        "passed": true,
        "reason": "Correct calculation",
        "suggestion": ""
    })
    .to_string();

    // (Synthesis 阶段的响应 - 最终输出)
    let synthesis_json = json!({
        "final_answer": 30.0,
        "notes": "Calculation successful"
    })
    .to_string();

    // 创建所有需要的 mock 对象
    let mock_planner = Arc::new(gemini3_flash::GeminiBackend::new());
    let mock_reviewer = Arc::new(gemini3_flash::GeminiBackend::new());

    // Verifier 会被调用 3 次
    let mock_verifier = Arc::new(gemini3_flash::GeminiBackend::new());

    // 根据代码逻辑，synthesis 使用的是 default_llm，所以我们需要将 synthesis 的响应放在 default_llm 中
    let mock_default = Arc::new(gemini3_flash::GeminiBackend::new());

    let store = Arc::new(local::LocalFileStore::new(PathBuf::from(
        "C:\\Users\\wang\\RustroverProjects\\Photon\\logs",
    )));

    // --- 2. 构建 Agent ---
    let mut agent = ManagerAgent::builder("TestAgent", mock_default, store)
        .with_planning_llm(mock_planner)
        .with_review_llm(mock_reviewer)
        .with_verification_llm(mock_verifier)
        .build();

    agent.register_skill(AddSkill);

    // --- 3. 运行测试 ---
    #[derive(Debug, Deserialize, PartialEq)]
    struct FinalOutput {
        final_answer: f64,
        notes: String,
    }

    let result: FinalOutput = agent
        .run_task("Calculate 10 + 20", "JSON with final_answer and notes")
        .await?;

    // --- 4. 断言结果 ---
    assert_eq!(result.final_answer, 30.0);
    assert_eq!(result.notes, "Calculation successful");

    Ok(())
}

#[tokio::test]
async fn test_dependency_resolution() -> Result<()> {
    // === 场景描述 ===
    // 测试参数传递机制 {{task_id.field}}
    // Task 1: Add(10, 20) -> returns { result: 30 }
    // Task 2: Add({{task_1.result}}, 5) -> returns { result: 35 }

    // 1. Planning Response
    let plan_json = json!({
        "thought": "Chained calculation",
        "tasks": [
            {
                "id": "task_1",
                "description": "Step 1",
                "skill_name": "add_numbers",
                "dependencies": [],
                "params": { "a": 10, "b": 20 }
            },
            {
                "id": "task_2",
                "description": "Step 2",
                "skill_name": "add_numbers",
                "dependencies": ["task_1"], // 依赖 task_1
                "params": { "a": "{{task_1.result}}", "b": 5 } // 参数引用
            }
        ]
    })
    .to_string();

    let review_json = plan_json.clone();

    let verification_pass = json!({ "passed": true, "reason": "ok", "suggestion": "" }).to_string();

    // Verify 会被调用 2次任务 * 3次投票 = 6次
    let mut verify_responses = vec![];
    for _ in 0..6 {
        verify_responses.push(verification_pass.clone());
    }

    let synthesis_json = json!({ "total": 35.0 }).to_string();

    let mock_planner = Arc::new(gemini3_flash::GeminiBackend::new());
    let mock_reviewer = Arc::new(gemini3_flash::GeminiBackend::new());
    let mock_verifier = Arc::new(gemini3_flash::GeminiBackend::new());
    let mock_default = Arc::new(gemini3_flash::GeminiBackend::new());

    let mut agent = ManagerAgent::builder(
        "DepAgent",
        mock_default,
        Arc::new(local::LocalFileStore::new(PathBuf::from(
            "C:\\Users\\wang\\RustroverProjects\\Photon\\logs",
        ))),
    )
    .with_planning_llm(mock_planner)
    .with_review_llm(mock_reviewer)
    .with_verification_llm(mock_verifier)
    .build();

    agent.register_skill(AddSkill);

    #[derive(Deserialize, Debug)]
    struct Res {
        total: f64,
    }

    let res: Res = agent.run_task("Chain calc", "").await?;

    assert_eq!(res.total, 35.0);

    Ok(())
}
