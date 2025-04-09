mod trigger;
use trigger::{decode_trigger_event, encode_trigger_output, Destination};
use wavs_wasi_chain::http::{fetch_json, http_request_get};
pub mod bindings;
use crate::bindings::{export, Guest, TriggerAction};
use serde::{Deserialize, Serialize};
use wstd::{http::HeaderValue, runtime::block_on};

struct Component;
export!(Component with_types_in bindings);

impl Guest for Component {
    fn run(action: TriggerAction) -> std::result::Result<Option<Vec<u8>>, String> {
        let (trigger_id, req, dest) =
            decode_trigger_event(action.data).map_err(|e| e.to_string())?;

        // Parse input - expects "GAME_ID|API_KEY"
        let input = std::str::from_utf8(&req).map_err(|e| e.to_string())?;
        println!("raw input: {}", input);

        let parts: Vec<&str> = input.split('|').collect();
        if parts.len() != 2 {
            return Err("Invalid input format. Expected 'GAME_ID|API_KEY'".to_string());
        }

        let game_id = parts[0];
        let api_key = parts[1];

        println!("game_id: {}", game_id);
        // Don't print API key for security reasons

        let res = block_on(async move {
            let scores_data = get_game_scores(game_id, api_key).await?;
            println!("scores_data: {:?}", scores_data);
            serde_json::to_vec(&scores_data).map_err(|e| e.to_string())
        })?;

        let output = match dest {
            Destination::Ethereum => Some(encode_trigger_output(trigger_id, &res)),
            Destination::CliOutput => Some(res),
        };
        Ok(output)
    }
}

async fn get_game_scores(game_id: &str, api_key: &str) -> Result<GameScoresData, String> {
    let url = format!(
        "https://api.sportradar.com/ncaamb/trial/v8/en/games/{}/boxscore.json?api_key={}",
        game_id, api_key
    );

    let mut req = http_request_get(&url).map_err(|e| e.to_string())?;
    req.headers_mut().insert("Accept", HeaderValue::from_static("application/json"));

    let json: SportRadarResponse = fetch_json(req).await.map_err(|e| e.to_string())?;

    Ok(GameScoresData {
        id: json.id,
        home_team: json.home.market.clone() + " " + &json.home.name,
        away_team: json.away.market.clone() + " " + &json.away.name,
        home_score: json.home.points,
        away_score: json.away.points,
        status: json.status,
    })
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GameScoresData {
    id: String,
    home_team: String,
    away_team: String,
    home_score: i32,
    away_score: i32,
    status: String,
}

// SportRadar API response structures
#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SportRadarResponse {
    pub id: String,
    pub title: String,
    pub status: String,
    pub scheduled: String,
    pub home: Team,
    pub away: Team,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Team {
    pub name: String,
    pub alias: String,
    pub market: String,
    pub id: String,
    pub points: i32,
}
