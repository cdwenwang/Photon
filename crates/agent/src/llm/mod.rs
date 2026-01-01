use anyhow::Result;
use async_trait::async_trait;
pub mod gemini3_flash;

/// 模型后端抽象接口 (ModelBackend)
///
/// 该 Trait 定义了与大语言模型 (LLM) 进行交互的通用行为。
///
/// # 线程安全
/// 该 Trait 继承了 `Send + Sync`，这意味着实现该 Trait 的对象可以在线程间安全地传递和共享。
/// 这对于在 Web 服务器（如 Axum, Actix-web）的状态中通过 `Arc<dyn ModelBackend>` 全局共享模型实例非常重要。
#[async_trait]
pub trait ModelBackend: Send + Sync {
    /// 执行一次对话请求 (Chat Completion)
    ///
    /// 这是一个异步方法，它将系统提示和用户输入发送给底层模型提供商，并等待完整的文本响应。
    ///
    /// # 参数 (Arguments)
    ///
    /// * `system_prompt` - 系统提示词 (System Prompt)。
    ///   用于设定模型的行为模式、角色扮演设定或前置上下文（例如："你是一个乐于助人的编程助手"）。
    ///
    /// * `user_input` - 用户输入 (User Prompt)。
    ///   用户实际发送的具体问题、指令或对话内容。
    ///
    /// # 返回值 (Returns)
    ///
    /// * `Ok(String)` - 如果请求成功，返回模型生成的回复文本。
    /// * `Err(anyhow::Error)` - 如果请求失败，返回包含错误详情的 `Result`。
    ///   常见错误可能包括：网络连接失败、API 密钥无效、超出 Token 限制等。
    async fn chat(&self, system_prompt: &str, user_input: &str) -> Result<String>;
}
