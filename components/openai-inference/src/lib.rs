use wavs_wasi_chain::http::{fetch_json, http_request_get, http_request_post_json};
pub mod bindings;
pub mod grok_types;
use crate::bindings::host::{log, LogLevel};
use crate::bindings::wavs::worker::layer_types::{TriggerData, TriggerDataEthContractEvent};
use crate::bindings::{export, Guest, TriggerAction};
use anyhow::Result;
use cid::Cid;
pub use grok_types::*;
use hex::FromHex;
use multihash::Multihash;
use serde_json::{to_string, Value};
use std::collections::HashMap;
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

// TODO: get the CID with the condition id (easily from event)

impl Guest for Component {
    fn run(action: TriggerAction) -> std::result::Result<Option<Vec<u8>>, String> {
        // Decode the trigger data
        log(LogLevel::Info, &format!("Run"));
        eprintln!("Run");
        let (trigger_id, req, dest) =
            decode_trigger_input(action.data).map_err(|e| e.to_string())?;

        // Convert bytes to string
        let input = std::str::from_utf8(&req).map_err(|e| e.to_string())?;
        log(LogLevel::Info, &format!("Received input: {}", input));
        eprintln!("input: {:?}", input);

        let parts: Vec<&str> = input.split('|').collect();
        if parts.len() != 4 {
            return Err("Input must be in format 'CONDITIONID|QUESTIONHASH|OPENAI_API_KEY|SEED'"
                .to_string());
        }

        let condition_id = parts[0];
        let cid = parts[1];
        let api_key = parts[2];
        let seed = parts[3].parse::<u64>().map_err(|_| "SEED must be an integer".to_string())?;

        //let cid = bytes32_to_cid_v1(condition_id).map_err(|e| e.to_string())?;
        log(LogLevel::Info, &format!("condition: {} -> CID: {:#?}", condition_id, cid));

        let res = block_on(async move {
            // get the IPFS file
            let qdata = get_question_data(&cid).await?;

            let question_data = QuestionInfo {
                question: qdata.question,
                description: qdata.description,
                results: qdata.results,
                is_scalar: qdata.is_scalar,
            };
            let resp_data: QuestionResponse = call_grok_api(&question_data, api_key, seed).await?;

            log(LogLevel::Info, &format!("Response data: {:#?}", resp_data));
            serde_json::to_vec(&resp_data).map_err(|e| e.to_string())
        })?;

        // prepare reportPayout tx args

        // Handle different destinations
        let output = match dest {
            Destination::Ethereum => Some(encode_trigger_output(trigger_id, &res)),
            Destination::CliOutput => Some(res),
        };

        Ok(output)
    }
}

pub async fn call_grok_api(
    data: &QuestionInfo,
    api_key: &str,
    seed: u64,
) -> Result<QuestionResponse, String> {
    let url = "https://api.x.ai/v1/chat/completions";

    // Define function schema for structured output
    let mut properties = HashMap::new();
    properties.insert(
        "answer".to_string(),
        ParameterProperty {
            prop_type: "string".to_string(),
            description: "String summarizing the predicted outcome".to_string(),
            items: None,
        },
    );
    properties.insert(
        "explanation".to_string(),
        ParameterProperty {
            prop_type: "string".to_string(),
            description: "Concise explanation of why the winner was chosen based on live data"
                .to_string(),
            items: None,
        },
    );
    properties.insert(
        "payoutVector".to_string(),
        ParameterProperty {
            prop_type: "array".to_string(),
            description: "Vector matching data.results length, with 1 for the definitive winner, proportional integers for scalar markets, or ones to cancel if no winner can be determined".to_string(),
            items: Some(ParameterItems {
                item_type: "integer".to_string(),
            }),
        },
    );
    properties.insert(
        "valid".to_string(),
        ParameterProperty {
            prop_type: "boolean".to_string(),
            description: "Whether live data is sufficient to determine the winner".to_string(),
            items: None,
        },
    );

    // Serialize QuestionInfo to JSON
    let data_json = to_string(data).map_err(|e| format!("Failed to serialize data: {}", e))?;

    log(LogLevel::Info, &format!("Data json: {}", data_json));

    let request = ChatCompletionRequest {
        model: "grok-3".to_string(),
        messages: vec![
            Message {
                role: "system".to_string(),
                content: r#"You are the oracle of a prediction market. 
                For the given question and description, search live data from reputable sources or trusted news to determine the definitive winner among the provided results.
                If no live data is found, set valid to false, return payoutVector as all ones, and explain why. 
                Answer format:
                {
                    answer: string summarizing the definitive winning outcome,
                    explanation: short concise explanation of why the winner was chosen based on live data,
                    payoutVector: an array of integers with length and order matching results, where >0 is set for the winner. 
                        For scalar markets (result is a number), use proportional integers (e.g., scaled to 0-100). 
                        To cancel a prediction (if no winner can be determined from live data), return an array of ones,
                        The payout values are calculated as index_value/sum(all index_value's) so use minimum integers.
                    valid: boolean indicating if live data is sufficient or if it should be consulted again later
                }"#.to_string(),
            },
            Message {
                role: "user".to_string(),
                content: format!("Data: {}", data_json),
            },
        ],
        temperature: 0.2,
        max_tokens: 100,
        seed,
        tools: Some(vec![Tool {
            tool_type: "function".to_string(),
            function: Function {
                name: "resolve_prediction".to_string(),
                description: "Resolves a prediction market with the definitive winner".to_string(),
                parameters: FunctionParameters {
                    param_type: "object".to_string(),
                    properties,
                    required: vec![
                        "answer".to_string(),
                        "explanation".to_string(),
                        "payoutVector".to_string(),
                        "valid".to_string(),
                    ],
                },
            },
        }]),
        tool_choice: Some(Value::Object(serde_json::from_str(r#"{"type": "function", "function": {"name": "resolve_prediction"}}"#).unwrap())),
    };

    let mut req = http_request_post_json(url, &request).map_err(|e| e.to_string())?;
    req.headers_mut().insert("Content-Type", HeaderValue::from_static("application/json"));
    req.headers_mut().insert(
        "Authorization",
        HeaderValue::from_str(&format!("Bearer {}", api_key)).map_err(|e| e.to_string())?,
    );

    let response: ChatCompletionResponse =
        fetch_json(req).await.map_err(|e| format!("Request failed: {}", e))?;

    eprintln!("Response: {:?}", response);

    if response.choices.is_empty() {
        return Err("No choices in response".to_string());
    }
    log(LogLevel::Info, &format!("response: {:#?}", response));

    // Extract tool call
    let choice = &response.choices[0];
    if let Some(content) = &choice.message.content {
        // Handle cases where no live data is found or outcome is undetermined
        if content.contains("no live data")
            || content.contains("impossible to determine")
            || content.contains("no verifiable information")
            || content.contains("no winner")
            || content.contains("no data")
        {
            return Ok(QuestionResponse {
                answer: String::from("No winner"),
                explanation: String::from(content),
                payout_vector: vec![1; data.results.len()],
                valid: false,
            });
        }
    }
    let tool_call =
        choice.message.tool_calls.as_ref().and_then(|calls| calls.get(0)).ok_or_else(|| {
            eprintln!("No tool calls; message: {:?}", choice.message);
            "No tool calls in response".to_string()
        })?;

    // Deserialize function call arguments into QuestionResponse
    let result: QuestionResponse =
        serde_json::from_str(&tool_call.function.arguments).map_err(|e| {
            eprintln!("Tool call arguments: {}", tool_call.function.arguments);
            format!("Failed to parse tool call arguments: {}", e)
        })?;

    // Validate payout_vector length and valid field
    if result.payout_vector.len() != data.results.len() {
        return Err(format!(
            "Payout vector length ({}) does not match results length ({})",
            result.payout_vector.len(),
            data.results.len()
        ));
    }
    if !result.valid && !result.payout_vector.iter().all(|&x| x == 1) {
        return Err(
            "Invalid response: valid is false but payout_vector is not all ones".to_string()
        );
    }
    Ok(result)
}

async fn get_question_data(cid: &str) -> Result<QuestionTestament, String> {
    let url = format!("https://ipfs.io/ipfs/{}", cid);
    println!("{}", url);

    let mut req = http_request_get(&url).map_err(|e| e.to_string())?;
    req.headers_mut().insert("Accept", HeaderValue::from_static("application/json"));

    let json: QuestionTestament = fetch_json(req).await.map_err(|e| e.to_string())?;
    Ok(json)
}
/*
only accepts strings without 0x
failed to calculate correctly the CID (but pinata is on the middle?)

fn bytes32_to_cid_v1(hex_digest: &str) -> Result<String, Box<dyn std::error::Error>> {
    // Remove 0x prefix if present
    let clean_hex = hex_digest.trim_start_matches("0x");

    // Decode to byte array
    let digest: Vec<u8> = Vec::from_hex(clean_hex)?;
    if digest.len() != 32 {
        return Err("Expected 32-byte digest".into());
    }

    // Wrap into a multihash using SHA2-256 (code 0x12)
    let mh = Multihash::<64>::wrap(0x12, &digest)?;

    // Create CIDv1 with raw codec (0x55)
    let cid = Cid::new_v1(0x55, mh);
    Ok(cid.to_string())
}
 */
