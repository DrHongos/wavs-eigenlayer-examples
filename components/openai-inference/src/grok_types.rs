use serde::{Deserialize, Serialize};
use std::collections::HashMap;
/*
// copied from https://docs.rs/x-ai/latest/src/x_ai/chat_compl.rs.html#11-39
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatCompletionRequest {
    pub model: String,
    pub messages: Vec<Message>,
    pub stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frequency_penalty: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub presence_penalty: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub n: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logprobs: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_logprobs: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seed: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logit_bias: Option<HashMap<u32, f32>>,
}
 */
#[derive(Serialize, Debug, Deserialize)]
pub struct QuestionTestament {
    pub question: String,
    pub oracle: String,
    pub description: String,
    pub results: Vec<String>,
    pub generated_at: String,
    pub is_scalar: bool,
}

// Input data struct
#[derive(Serialize, Debug)]
pub struct QuestionInfo {
    pub question: String,
    pub description: String,
    pub results: Vec<String>,
    pub is_scalar: bool,
}

// Expected response struct
#[derive(Deserialize, Serialize, Debug)]
pub struct QuestionResponse {
    pub answer: String,
    pub explanation: String,
    #[serde(rename = "payoutVector")]
    pub payout_vector: Vec<u8>,
    pub valid: bool,
}

// Request Structs
#[derive(Serialize, Debug)]
pub struct ChatCompletionRequest {
    pub model: String,
    pub messages: Vec<Message>,
    pub temperature: f32,
    pub max_tokens: u32,
    pub seed: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<Tool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<serde_json::Value>,
}

#[derive(Serialize, Debug)]
pub struct Message {
    pub role: String,
    pub content: String,
}

#[derive(Serialize, Debug)]
pub struct Tool {
    #[serde(rename = "type")]
    pub tool_type: String,
    pub function: Function,
}

#[derive(Serialize, Debug)]
pub struct Function {
    pub name: String,
    pub description: String,
    pub parameters: FunctionParameters,
}

#[derive(Serialize, Debug)]
pub struct FunctionParameters {
    #[serde(rename = "type")]
    pub param_type: String,
    pub properties: HashMap<String, ParameterProperty>,
    pub required: Vec<String>,
}

#[derive(Serialize, Debug)]
pub struct ParameterProperty {
    #[serde(rename = "type")]
    pub prop_type: String,
    pub description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub items: Option<ParameterItems>,
}

#[derive(Serialize, Debug)]
pub struct ParameterItems {
    #[serde(rename = "type")]
    pub item_type: String,
}

// Response Structs
#[derive(Deserialize, Debug)]
pub struct ChatCompletionResponse {
    pub id: String,
    pub object: String,
    pub created: u64,
    pub model: String,
    pub choices: Vec<Choice>,
    pub usage: Usage,
}

#[derive(Deserialize, Debug)]
pub struct Choice {
    pub index: u32,
    pub message: ResponseMessage,
    pub finish_reason: String,
}

#[derive(Deserialize, Debug)]
pub struct ResponseMessage {
    pub role: String,
    #[serde(default)]
    pub content: Option<String>,
    #[serde(default)]
    pub tool_calls: Option<Vec<ToolCall>>,
}

#[derive(Deserialize, Debug)]
pub struct ToolCall {
    pub id: String,
    #[serde(rename = "type")]
    pub tool_type: String,
    pub function: FunctionCall,
}

#[derive(Deserialize, Debug)]
pub struct FunctionCall {
    pub name: String,
    pub arguments: String, // JSON string, to be deserialized into QuestionResponse
}

// Usage information
#[derive(Deserialize, Debug)]
pub struct Usage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

// Error handling
#[derive(thiserror::Error, Debug)]
pub enum ApiError {
    #[error("HTTP request failed: {0}")]
    RequestError(String),
    #[error("JSON serialization/deserialization failed: {0}")]
    JsonError(#[from] serde_json::Error),
    #[error("API returned an error: {0}")]
    ApiError(String),
}
