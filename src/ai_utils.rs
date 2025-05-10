use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

// 定义发送到Ollama /v1/chat/completions端点的请求体结构体
#[derive(Serialize, Debug, Clone)]
pub struct OpenAIChatRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    pub temperature: Option<f32>, // OpenAI API通常将temperature作为可选的顶层参数
    pub stream: bool,
    // 你可以在这里添加其他OpenAI支持的选项，例如 top_p, max_tokens 等
    // pub max_tokens: Option<u32>,
    // pub top_p: Option<f32>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct OpenAIMessage {
    pub role: String,
    pub content: String,
}

#[derive(Deserialize, Debug, Clone)]
pub struct OpenAIChoice {
    pub index: u32,
    pub message: OpenAIMessage,
    pub finish_reason: String,
    // pub logprobs: Option<serde_json::Value>, // 如果需要解析logprobs
}

#[derive(Deserialize, Debug, Clone)]
pub struct OpenAIUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

#[derive(Deserialize, Debug, Clone)]
pub struct OpenAIChatCompletionResponse {
    pub id: String,
    pub object: String,
    pub created: i64, // 通常是Unix时间戳
    pub model: String,
    pub system_fingerprint: Option<String>, // 根据您的示例，这个字段存在
    pub choices: Vec<OpenAIChoice>,
    pub usage: OpenAIUsage,
}