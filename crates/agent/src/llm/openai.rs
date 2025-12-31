use super::ModelBackend;
use anyhow::{Context, Result};
use async_openai::{
    config::OpenAIConfig,
    types::{
        ChatCompletionRequestSystemMessageArgs, ChatCompletionRequestUserMessageArgs,
        CreateChatCompletionRequestArgs,
    },
    Client,
};
use async_trait::async_trait;
// 假设你的 Trait 定义在 mod.rs 中

pub struct OpenAiBackend {
    client: Client<OpenAIConfig>,
    model: String,
}

impl OpenAiBackend {
    /// 从环境变量 OPENAI_API_KEY 初始化
    pub fn new(model: &str) -> Self {
        let config = OpenAIConfig::new(); // 自动读取 env: OPENAI_API_KEY
        let client = Client::with_config(config);
        Self {
            client,
            model: model.to_string(),
        }
    }

    /// 支持自定义 BaseUrl (例如对接 DeepSeek, OneAPI)
    pub fn new_with_base_url(api_key: &str, base_url: &str, model: &str) -> Self {
        let config = OpenAIConfig::new()
            .with_api_key(api_key)
            .with_api_base(base_url);
        let client = Client::with_config(config);
        Self {
            client,
            model: model.to_string(),
        }
    }
}

#[async_trait]
impl ModelBackend for OpenAiBackend {
    async fn chat(&self, system_prompt: &str, user_input: &str) -> Result<String> {
        // 1. 构建 System Message
        let system_msg = ChatCompletionRequestSystemMessageArgs::default()
            .content(system_prompt)
            .build()?;

        // 2. 构建 User Message
        let user_msg = ChatCompletionRequestUserMessageArgs::default()
            .content(user_input)
            .build()?;

        // 3. 构建请求
        let request = CreateChatCompletionRequestArgs::default()
            .model(&self.model)
            .messages([system_msg.into(), user_msg.into()])
            // 设置 temperature=0 让 JSON 输出更稳定
            .temperature(0.0)
            .build()?;

        // 4. 发送请求
        tracing::debug!("Sending request to LLM model: {}", self.model);
        let response = self
            .client
            .chat()
            .create(request)
            .await
            .context("Failed to call OpenAI API")?;

        // 5. 提取内容
        let content = response
            .choices
            .first()
            .and_then(|c| c.message.content.clone())
            .unwrap_or_default();

        Ok(content)
    }
}
