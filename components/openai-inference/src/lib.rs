use wavs_wasi_chain::http::{fetch_json, http_request_post_json};
pub mod bindings;
use crate::bindings::host::{log, LogLevel};
use crate::bindings::wavs::worker::layer_types::{TriggerData, TriggerDataEthContractEvent};
use crate::bindings::{export, Guest, TriggerAction};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fmt;
use wstd::{http::HeaderValue, runtime::block_on};

struct Component;
export!(Component with_types_in bindings);

// Define destination enum for the output
enum Destination {
    Ethereum,
    CliOutput,
}

// Function to decode trigger event data
fn decode_trigger_input(trigger_data: TriggerData) -> Result<(u64, Vec<u8>, Destination)> {
    match trigger_data {
        TriggerData::EthContractEvent(TriggerDataEthContractEvent { log, .. }) => {
            // This would be used for Ethereum event triggers
            // For simplicity, we're not implementing the full Ethereum event handling
            Err(anyhow::anyhow!("Ethereum event triggers not supported"))
        }
        TriggerData::Raw(data) => Ok((0, data.clone(), Destination::CliOutput)),
        _ => Err(anyhow::anyhow!("Unsupported trigger data type")),
    }
}

// Function to encode trigger output for Ethereum
fn encode_trigger_output(trigger_id: u64, output: impl AsRef<[u8]>) -> Vec<u8> {
    // For simplicity, we're just returning the output as is
    // In a real implementation, this would encode the output for Ethereum
    output.as_ref().to_vec()
}

impl Guest for Component {
    fn run(action: TriggerAction) -> std::result::Result<Option<Vec<u8>>, String> {
        // Decode the trigger data
        let (trigger_id, req, dest) =
            decode_trigger_input(action.data).map_err(|e| e.to_string())?;

        // Convert bytes to string
        let input = std::str::from_utf8(&req).map_err(|e| e.to_string())?;
        log(LogLevel::Info, &format!("Received input: {}", input));

        // Parse input in format 'PROMPT|OPENAI_API_KEY|SEED'
        let parts: Vec<&str> = input.split('|').collect();
        if parts.len() != 3 {
            return Err("Input must be in format 'PROMPT|OPENAI_API_KEY|SEED'".to_string());
        }

        let prompt = parts[0];
        let api_key = parts[1];
        let seed = parts[2].parse::<u64>().map_err(|_| "SEED must be an integer".to_string())?;

        let res = block_on(async move {
            let resp_data = call_openai_api(prompt, api_key, seed).await?;
            log(LogLevel::Info, &format!("Response data: {}", resp_data));
            serde_json::to_vec(&resp_data).map_err(|e| e.to_string())
        })?;

        // Handle different destinations
        let output = match dest {
            Destination::Ethereum => Some(encode_trigger_output(trigger_id, &res)),
            Destination::CliOutput => Some(res),
        };

        Ok(output)
    }
}

async fn call_openai_api(
    prompt: &str,
    api_key: &str,
    seed: u64,
) -> Result<OpenAIResponseData, String> {
    let url = "https://api.openai.com/v1/chat/completions";

    // Create the request payload
    let request_payload = OpenAIRequest {
        seed,
        model: "gpt-4o".to_string(),
        messages: vec![
            Message {
                role: "system".to_string(),
                content: "You are a helpful assistant.".to_string(),
            },
            Message { role: "user".to_string(), content: prompt.to_string() },
        ],
    };

    log(LogLevel::Info, &format!("Request payload: {:?}", request_payload));

    // Create the HTTP request
    let mut req = http_request_post_json(url, &request_payload).map_err(|e| e.to_string())?;

    // Add headers
    req.headers_mut().insert("Accept", HeaderValue::from_static("application/json"));
    req.headers_mut().insert("Content-Type", HeaderValue::from_static("application/json"));
    req.headers_mut().insert(
        "Authorization",
        HeaderValue::from_str(&format!("Bearer {}", api_key)).map_err(|e| e.to_string())?,
    );

    // Make the request and get the response
    let response: OpenAIResponse = fetch_json(req).await.map_err(|e| e.to_string())?;

    // Check for API errors
    if let Some(error) = response.error {
        return Err(format!("OpenAI API error: {}", error.message));
    }

    // Extract the content and finish reason from the response
    if response.choices.is_empty() {
        return Err("No choices returned from OpenAI API".to_string());
    }

    let choice = &response.choices[0];

    // Extract content from the message field
    let content = choice
        .message
        .get("content")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "Could not extract content from message".to_string())?;

    let finish_reason = choice.finish_reason.clone().unwrap_or_else(|| "unknown".to_string());

    Ok(OpenAIResponseData { content: content.to_string(), finish_reason })
}

#[derive(Debug, Serialize)]
struct OpenAIRequest {
    seed: u64,
    model: String,
    messages: Vec<Message>,
}

#[derive(Debug, Serialize)]
struct Message {
    role: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct OpenAIResponse {
    id: Option<String>,
    model: Option<String>,
    choices: Vec<OpenAIChoice>,
    error: Option<OpenAIError>, // NOT flattened
}

#[derive(Debug, Deserialize)]
struct OpenAIChoice {
    message: serde_json::Value, // Use Value for flexibility
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OpenAIError {
    message: String,
    #[serde(rename = "type")]
    error_type: Option<String>,
    code: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OpenAIResponseData {
    content: String,
    finish_reason: String,
}

// Implement Display trait for OpenAIResponseData
impl fmt::Display for OpenAIResponseData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Content: {}, Finish Reason: {}", self.content, self.finish_reason)
    }
}
