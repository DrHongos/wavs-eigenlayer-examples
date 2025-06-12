mod trigger;
use trigger::{decode_trigger_event, encode_trigger_output, Destination};
use wavs_wasi_chain::http::{fetch_json, http_request_get};
pub mod bindings;
use crate::bindings::{export, Guest, TriggerAction};
use rhai::{Dynamic, Engine, Map, Scope};
use serde::{Deserialize, Serialize};
use wstd::{http::HeaderValue, runtime::block_on};

struct Component;
export!(Component with_types_in bindings);

// TODO:
// https://api.sportradar.com/soccer/trial/v4/openapi/swagger/index.html
// store GAME_ID (and read it) from IPFS CIDs w/ arbitrary logic
// use rhai to execute logic https://github.com/rhaiscript/rhai

// change response to be reportPayout
// create submitter & trigger to call for this

// how to handle trigger (besides checking the match is complete) but to create the service close to the endtime

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
        // add questionId or IPFS CID

        println!("game_id: {}", game_id);
        // Don't print API key for security reasons

        // TODO:
        // get logic from IPFS
        // check logic with API data,
        // prepare answer
        let res = block_on(async move {
            let game_data = get_game_data(game_id, api_key).await?;
            //println!("scores_data: {:?}", game_data);
            // simple logic asked to chatgpt for a match winner when its finished
            let logic = r#"
            if match_status != "ended" && status != "closed" {
                throw("Match has not ended yet");
            }

            if home_score > away_score {
                #{ outcome: home_team, payout: [1, 0, 0] }
            } else if away_score > home_score {
                #{ outcome: away_team, payout: [0, 1, 0] }
            } else {
                #{ outcome: "draw", payout: [0, 0, 1] }
            }
            "#;

            if let Ok(res) = evaluate_rhai_script(&game_data, logic) {
                let (winner, payout) = res;
                println!("Winner: {}", winner);
                println!("Payout: {:?}", payout);
            }
            serde_json::to_vec(&game_data).map_err(|e| e.to_string())
        })?;

        let output = match dest {
            Destination::Ethereum => Some(encode_trigger_output(trigger_id, &res)),
            Destination::CliOutput => Some(res),
        };
        Ok(output)
    }
}
/*
async fn get_question_data(cid: &str) -> Result<String, String> {
    let url = format!("https://ipfs.io/{}", cid);
    println!("{}", url);

    let mut req = http_request_get(&url).map_err(|e| e.to_string())?;
    req.headers_mut().insert("Accept", HeaderValue::from_static("application/json"));

    //let json: Question = fetch_json(req).await.map_err(|e| e.to_string())?;
    Ok(String::new())
}
 */
async fn get_game_data(game_id: &str, api_key: &str) -> Result<MatchResult, String> {
    let url = format!(
        "https://api.sportradar.com/soccer/trial/v4/en/sport_events/{}/summary.json?api_key={}",
        game_id, api_key
    );
    //println!("Request URL: {}", url);

    let mut req = http_request_get(&url).map_err(|e| e.to_string())?;
    req.headers_mut().insert("Accept", HeaderValue::from_static("application/json"));

    let data: MatchResult = fetch_json(req).await.map_err(|e| e.to_string())?;
    //println!("{:#?}", json);
    Ok(data)
}

fn evaluate_rhai_script(
    data: &MatchResult,
    script: &str,
) -> Result<(String, Vec<u8>), Box<rhai::EvalAltResult>> {
    let engine = Engine::new();

    let mut scope = build_rhai_scope(data);

    let result: Dynamic = engine.eval_with_scope(&mut scope, &script)?;
    let map = result.clone_cast::<Map>();

    let outcome = map
        .get("outcome")
        .and_then(|v| v.clone().try_cast::<String>())
        .ok_or("Missing or invalid outcome")?;

    let payout = map
        .get("payout")
        .and_then(|v| v.clone().try_cast::<rhai::Array>())
        .ok_or("Missing or invalid payout")?;

    let payout_vec: Vec<u8> = payout.into_iter().map(|v| v.as_int().unwrap_or(0) as u8).collect();
    Ok((outcome, payout_vec))
}

pub fn build_rhai_scope(data: &MatchResult) -> Scope {
    let mut scope = Scope::new();

    let status = &data.sport_event_status;
    let event = &data.sport_event;
    let context = &event.sport_event_context;

    // Scores
    scope.push("home_score", status.home_score);
    scope.push("away_score", status.away_score);

    // Winner ID (may be null)
    if let Some(winner_id) = &status.winner_id {
        scope.push("winner_id", winner_id.clone());
    }

    // Match status (e.g., "ended", "closed")
    scope.push("match_status", status.match_status.clone());
    scope.push("status", status.status.clone());

    // Competitors (IDs and names)
    if let Some(home) = event.competitors.iter().find(|c| c.qualifier == "home") {
        scope.push("home_team", home.name.clone());
        scope.push("home_team_id", home.id.clone());
    }

    if let Some(away) = event.competitors.iter().find(|c| c.qualifier == "away") {
        scope.push("away_team", away.name.clone());
        scope.push("away_team_id", away.id.clone());
    }

    // Basic match info
    scope.push("start_time", event.start_time.clone());
    scope.push("match_id", event.id.clone());
    scope.push("confirmed", event.start_time_confirmed);

    // Contextual info
    scope.push("competition", context.competition.name.clone());
    scope.push("competition_id", context.competition.id.clone());
    scope.push("season", context.season.name.clone());
    scope.push("stage", context.stage.phase.clone());
    scope.push("category", context.category.name.clone());
    scope.push("country_code", context.category.country_code.clone());

    // Round number (if present)
    if let Some(round) = &context.round {
        scope.push("round_number", round.number);
    }

    // Group (optional)
    if let Some(group) = context.groups.get(0) {
        scope.push("group", group.group_name.clone());
    }

    // Weather / Conditions (optional)
    if let Some(conditions) = &event.sport_event_conditions {
        if let Some(weather) = &conditions.weather {
            scope.push("pitch_conditions", weather.pitch_conditions.clone());
            scope.push("overall_conditions", weather.overall_conditions.clone());
        }
        if let Some(ground) = &conditions.ground {
            scope.push("neutral_ground", ground.neutral);
        }
    }

    scope
}

// chatgpt structures for soccer api
impl SportEvent {
    pub fn team_by_qualifier(&self, role: &str) -> Option<&Competitor> {
        self.competitors.iter().find(|c| c.qualifier == role)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MatchResult {
    pub generated_at: String,
    pub sport_event: SportEvent,
    pub sport_event_status: SportEventStatus,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SportEvent {
    pub id: String,
    pub start_time: String,
    pub start_time_confirmed: bool,
    pub sport_event_context: SportEventContext,
    pub coverage: Coverage,
    pub competitors: Vec<Competitor>,
    pub sport_event_conditions: Option<SportEventConditions>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SportEventContext {
    pub sport: NamedEntity,
    pub category: Category,
    pub competition: Competition,
    pub season: Season,
    pub stage: Stage,
    pub round: Option<Round>,
    pub groups: Vec<Group>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct NamedEntity {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Category {
    pub id: String,
    pub name: String,
    pub country_code: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Competition {
    pub id: String,
    pub name: String,
    pub gender: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Season {
    pub id: String,
    pub name: String,
    pub start_date: String,
    pub end_date: String,
    pub year: String,
    pub competition_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Stage {
    pub order: u32,
    #[serde(rename = "type")]
    pub stage_type: String,
    pub phase: String,
    pub start_date: String,
    pub end_date: String,
    pub year: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Round {
    pub number: u32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Group {
    pub id: String,
    pub name: String,
    pub group_name: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Coverage {
    #[serde(rename = "type")]
    pub coverage_type: String,
    pub sport_event_properties: CoverageProperties,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CoverageProperties {
    pub lineups: bool,
    pub formations: bool,
    pub venue: bool,
    pub extended_player_stats: bool,
    pub extended_team_stats: bool,
    pub ballspotting: bool,
    pub commentary: bool,
    pub fun_facts: bool,
    pub goal_scorers: bool,
    pub goal_scorers_live: bool,
    pub scores: String,
    pub game_clock: bool,
    pub deeper_play_by_play: bool,
    pub deeper_player_stats: bool,
    pub deeper_team_stats: bool,
    pub basic_play_by_play: bool,
    pub basic_player_stats: bool,
    pub basic_team_stats: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Competitor {
    pub id: String,
    pub name: String,
    pub country: String,
    pub country_code: String,
    pub abbreviation: String,
    pub qualifier: String,
    pub gender: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SportEventConditions {
    pub weather: Option<Weather>,
    pub ground: Option<Ground>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Weather {
    pub pitch_conditions: String,
    pub overall_conditions: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Ground {
    pub neutral: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SportEventStatus {
    pub status: String,
    pub match_status: String,
    pub home_score: i32,
    pub away_score: i32,
    pub winner_id: Option<String>,
    pub period_scores: Vec<PeriodScore>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PeriodScore {
    pub home_score: i32,
    pub away_score: i32,
    #[serde(rename = "type")]
    pub period_type: String,
    pub number: u8,
}
