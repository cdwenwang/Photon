use crate::llm::ModelBackend;
use anyhow::{Context, Result};
use async_trait::async_trait;
use dotenvy::dotenv;
use reqwest::header;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::time::Duration;
use tokio::sync::OnceCell;

static GEMINI_LLM: OnceCell<GeminiBackend> = OnceCell::const_new();

pub async fn llm() -> &'static GeminiBackend {
    GEMINI_LLM
        .get_or_init(|| async { GeminiBackend::new() })
        .await
}

#[derive(Debug, Deserialize)]
struct GeminiResponse {
    // 对应 JSON 中的 "candidates"
    candidates: Option<Vec<Candidate>>,
}

#[derive(Debug, Deserialize)]
struct Candidate {
    // 对应 JSON 中的 "content"
    content: Content,
    // 对应 JSON 中的 "finishReason" (可选，用于调试)
    #[serde(rename = "finishReason")]
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Content {
    // 对应 JSON 中的 "parts"
    parts: Vec<Part>,
}

#[derive(Debug, Deserialize)]
struct Part {
    // 对应 JSON 中的 "text"
    text: String,
}

// ==========================================
// 2. 实现 Gemini Backend
// ==========================================

pub struct GeminiBackend {
    model: String,
    client: reqwest::Client,
}

impl GeminiBackend {
    /// 创建一个新的 Gemini 实例
    pub fn new() -> Self {
        dotenv().ok();
        // 从环境变量获取 KEY，避免硬编码
        let api_key = std::env::var("GEMINI_API_KEY").unwrap_or("YOUR_TEST_KEY".to_string());
        let model = "gemini-3-flash-preview".to_string();
        // 构建默认 Header，包含 API Key (也可以在每次请求的 URL query 中传，这里按 Header 方式)
        let mut headers = header::HeaderMap::new();
        headers.insert(
            "x-goog-api-key",
            header::HeaderValue::from_str(&api_key).unwrap_or(header::HeaderValue::from_static("")),
        );
        headers.insert(
            "Content-Type",
            header::HeaderValue::from_static("application/json"),
        );

        let client = reqwest::Client::builder()
            .default_headers(headers)
            .timeout(Duration::from_secs(60)) // 设置超时
            .build()
            .expect("Failed to build reqwest client");

        Self { client, model }
    }
}

#[async_trait]
impl ModelBackend for GeminiBackend {
    async fn chat(&self, system_prompt: &str, user_input: &str) -> Result<String> {
        // 1. 构建 URL
        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent",
            self.model
        );

        // 2. 构建请求 Body
        // Gemini 支持 system_instruction 字段，但为了兼容性且简单起见，
        // 我们这里将 system_prompt 和 user_input 合并，或者利用 system_instruction (推荐)。
        // 下面展示使用标准的 contents 结构，并通过 system_instruction 字段传入系统提示。

        let request_body = json!({
            "system_instruction": {
                "parts": [{ "text": system_prompt }]
            },
            "contents": [{
                "role": "user",
                "parts": [{ "text": user_input }]
            }]
        });

        // 3. 发送异步请求
        let res = self
            .client
            .post(&url)
            .json(&request_body)
            .send()
            .await
            .context("Failed to send request to Gemini API")?;

        // 4. 检查 HTTP 状态码
        if !res.status().is_success() {
            let status = res.status();
            let error_text = res.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!(
                "Gemini API Error: Status {}, Body: {}",
                status,
                error_text
            ));
        }

        // 5. 解析 JSON 响应
        let response_data: GeminiResponse = res
            .json()
            .await
            .context("Failed to deserialize Gemini response JSON")?;

        // 6. 提取文本内容
        // 路径: candidates[0] -> content -> parts[0] -> text
        if let Some(candidates) = response_data.candidates {
            if let Some(first_candidate) = candidates.first() {
                if let Some(first_part) = first_candidate.content.parts.first() {
                    return Ok(first_part.text.clone());
                }
            }
        }

        Err(anyhow::anyhow!(
            "Response JSON structure mismatch: No valid text found in candidates"
        ))
    }
}

// ==========================================
// 3. 单元测试示例
// ==========================================

#[cfg(test)]
mod tests {
    use super::*;

    // 注意：运行此测试需要真实的网络和 API Key，或者你可以 Mock Server。
    // 这里仅演示如何调用。
    #[tokio::test]
    async fn test_gemini_backend_implementation() {
        let backend = llm().await;

        let system = "你是一个精通Rust的助手，回答要简洁。";
        let user = "如何声明一个不可变变量？";

        // 实际调用 (如果 key 无效会报错)
        match backend.chat(system, user).await {
            Ok(response) => {
                println!("Gemini Response: {}", response);
                assert!(!response.is_empty());
            }
            Err(e) => {
                println!("Test skipped or failed (expected if no valid key): {:?}", e);
            }
        }
    }
}
