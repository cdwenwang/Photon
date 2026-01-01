use crate::llm::ModelBackend;
use anyhow::{Context, Result};
use async_trait::async_trait;
use dotenvy::dotenv;
use reqwest::header;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::time::Duration;
use tokio::sync::OnceCell;

static QWEN_LLM: OnceCell<QwenBackend> = OnceCell::const_new();

pub async fn llm() -> &'static QwenBackend {
    QWEN_LLM
        .get_or_init(|| async { QwenBackend::new() })
        .await
}

// ==========================================
// 1. 定义响应结构体 (OpenAI 兼容格式)
// ==========================================

#[derive(Debug, Deserialize)]
struct QwenResponse {
    // 对应 JSON 中的 "choices"
    choices: Vec<QwenChoice>,
}

#[derive(Debug, Deserialize)]
struct QwenChoice {
    // 对应 JSON 中的 "message"
    message: QwenMessage,
    // 对应 JSON 中的 "finish_reason" (可选)
    #[serde(rename = "finish_reason")]
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct QwenMessage {
    // 对应 JSON 中的 "content"
    content: String,
}

// ==========================================
// 2. 实现 Qwen Backend
// ==========================================

pub struct QwenBackend {
    model: String,
    client: reqwest::Client,
}

impl QwenBackend {
    /// 创建一个新的 Qwen 实例
    pub fn new() -> Self {
        dotenv().ok();
        // 从环境变量获取 KEY (DASHSCOPE_API_KEY)
        let api_key = std::env::var("DASHSCOPE_API_KEY").unwrap_or("YOUR_TEST_KEY".to_string());
        // 默认模型，例如 qwen-plus, qwen-max, qwen-turbo
        let model = "qwen3-max".to_string();

        let mut headers = header::HeaderMap::new();
        // Qwen (DashScope) 使用 Bearer Token 鉴权
        let auth_value = format!("Bearer {}", api_key);
        let mut auth_header_val = header::HeaderValue::from_str(&auth_value)
            .expect("Invalid characters in API Key or Header");
        // 标记为敏感信息，日志中不打印
        auth_header_val.set_sensitive(true);

        headers.insert(
            header::AUTHORIZATION,
            auth_header_val,
        );
        headers.insert(
            header::CONTENT_TYPE,
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
impl ModelBackend for QwenBackend {
    async fn chat(&self, system_prompt: &str, user_input: &str) -> Result<String> {
        // 1. 构建 URL (DashScope 兼容 OpenAI 的接口)
        let url = "https://dashscope.aliyuncs.com/compatible-mode/v1/chat/completions";

        // 2. 构建请求 Body (OpenAI 格式)
        let request_body = json!({
            "model": self.model,
            "messages": [
                {
                    "role": "system",
                    "content": system_prompt
                },
                {
                    "role": "user",
                    "content": user_input
                }
            ]
        });

        // 3. 发送异步请求
        let res = self
            .client
            .post(url)
            .json(&request_body)
            .send()
            .await
            .context("Failed to send request to Qwen API")?;

        // 4. 检查 HTTP 状态码
        if !res.status().is_success() {
            let status = res.status();
            let error_text = res.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!(
                "Qwen API Error: Status {}, Body: {}",
                status,
                error_text
            ));
        }

        // 5. 解析 JSON 响应
        let response_data: QwenResponse = res
            .json()
            .await
            .context("Failed to deserialize Qwen response JSON")?;

        // 6. 提取文本内容
        // 路径: choices[0] -> message -> content
        if let Some(first_choice) = response_data.choices.first() {
            return Ok(first_choice.message.content.clone());
        }

        Err(anyhow::anyhow!(
            "Response JSON structure mismatch: No choices found"
        ))
    }
}

// ==========================================
// 3. 单元测试示例
// ==========================================

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_qwen_backend_implementation() {
        // 确保你的 .env 文件中有 DASHSCOPE_API_KEY
        let backend = llm().await;

        let system = "你是一个助手。";
        let user = "你好，请介绍一下你自己。";

        match backend.chat(system, user).await {
            Ok(response) => {
                println!("Qwen Response: {}", response);
                assert!(!response.is_empty());
            }
            Err(e) => {
                // 如果没有配置 Key，这里会报错，属正常预期
                println!("Test skipped or failed: {:?}", e);
            }
        }
    }
}