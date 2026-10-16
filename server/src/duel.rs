//! A scripted Fighter that plays the `duel` scene through the JSON API and
//! writes a transcript citing every engine roll. Used by the `nat20_duel`
//! binary and the integration tests.

use serde_json::{Value, json};

use crate::{log::LogEntry, session::Session};

/// The scripted Fighter plays one turn at a time:
/// Second Wind when at half HP or less, otherwise attack with the longsword,
/// Action Surge once for a second attack, then end the turn. Reactions are
/// always taken.
pub fn run_duel() -> (String, Vec<LogEntry>) {
    let mut session = Session::default();
    let mut transcript = String::new();
    let fighter = "bren";

    let mut response = call(
        &mut session,
        &mut transcript,
        json!({ "cmd": "load_scene", "scene_id": "duel" }),
    );
    let mut surged_this_turn = false;
    let mut last_round = 0;

    for _ in 0..200 {
        let awaiting = response["awaiting"].clone();
        match awaiting["kind"].as_str() {
            Some("combat_over") | Some("nothing") | Some("no_scene") | None => break,
            Some("reaction") => {
                let actor = awaiting["actor"].as_str().unwrap().to_string();
                response = call(
                    &mut session,
                    &mut transcript,
                    json!({ "cmd": "react", "actor": actor, "choice": 0 }),
                );
            }
            Some("action") => {
                let state = call_quiet(&mut session, json!({ "cmd": "get_state" }));
                let round = state["result"]["round"].as_u64().unwrap_or(0);
                if round != last_round {
                    surged_this_turn = false;
                    last_round = round;
                }
                let me = state["result"]["creatures"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|c| c["handle"] == fighter)
                    .cloned()
                    .unwrap();
                let hp = me["hp"].as_u64().unwrap_or(0);
                let max_hp = me["max_hp"].as_u64().unwrap_or(1);

                let legal = call(
                    &mut session,
                    &mut transcript,
                    json!({ "cmd": "legal_actions", "actor": fighter }),
                );
                let actions = legal["result"]["actions"]
                    .as_array()
                    .cloned()
                    .unwrap_or_default();
                let find = |id: &str| actions.iter().find(|a| a["action"] == id).cloned();

                let second_wind = find("nat20_core::action.fighter.second_wind");
                let surge = find("nat20_core::action.fighter.action_surge");
                let attack = find("nat20_core::action.melee_attack").and_then(|a| {
                    a["options"].as_array().and_then(|opts| {
                        opts.iter()
                            .find(|o| o["weapon"] == "longsword")
                            .or_else(|| opts.first())
                            .cloned()
                    })
                });

                let request = if hp * 2 <= max_hp && second_wind.is_some() {
                    json!({ "cmd": "submit", "actor": fighter, "action": "nat20_core::action.fighter.second_wind" })
                } else if let Some(option) = attack {
                    json!({
                        "cmd": "submit",
                        "actor": fighter,
                        "action": "nat20_core::action.melee_attack",
                        "context": option["context"],
                        "targets": ["goblin"],
                    })
                } else if surge.is_some() && !surged_this_turn {
                    surged_this_turn = true;
                    json!({ "cmd": "submit", "actor": fighter, "action": "nat20_core::action.fighter.action_surge" })
                } else {
                    json!({ "cmd": "end_turn", "actor": fighter })
                };
                response = call(&mut session, &mut transcript, request);
                if response["ok"] == false {
                    response = call(
                        &mut session,
                        &mut transcript,
                        json!({ "cmd": "end_turn", "actor": fighter }),
                    );
                }
            }
            Some(other) => panic!("unexpected awaiting kind {other}"),
        }
    }

    let final_state = call_quiet(&mut session, json!({ "cmd": "get_state" }));
    transcript.push_str("\nFinal state:\n");
    for c in final_state["result"]["creatures"].as_array().unwrap() {
        transcript.push_str(&format!(
            "  {} ({}): HP {}/{}, AC {}, {}\n",
            c["handle"].as_str().unwrap_or("?"),
            c["name"].as_str().unwrap_or("?"),
            c["hp"],
            c["max_hp"],
            c["ac"],
            c["life_state"].as_str().unwrap_or("?"),
        ));
    }
    (transcript, session.log().to_vec())
}

fn call_quiet(session: &mut Session, request: Value) -> Value {
    serde_json::from_str(&session.handle_json(&request.to_string())).unwrap()
}

/// Sends a request, echoes it and the log entries it produced into the
/// transcript. Roll entries are written with their engine citation.
fn call(session: &mut Session, transcript: &mut String, request: Value) -> Value {
    if request["cmd"] != "legal_actions" {
        transcript.push_str(&format!("  api> {request}\n"));
    }
    let response = call_quiet(session, request.clone());
    if response["ok"] == false {
        transcript.push_str(&format!(
            "  api! rejected: {}\n",
            response["error"]["message"]
        ));
    }
    if request["cmd"] == "legal_actions" {
        let names: Vec<String> = response["result"]["actions"]
            .as_array()
            .map(|a| {
                a.iter()
                    .map(|x| x["name"].as_str().unwrap_or("?").to_string())
                    .collect()
            })
            .unwrap_or_default();
        transcript.push_str(&format!(
            "  options for {}: {}\n",
            request["actor"].as_str().unwrap_or("?"),
            names.join(", ")
        ));
    }
    for entry in response["new_events"].as_array().unwrap() {
        let text = entry["text"].as_str().unwrap_or("");
        match entry["kind"].as_str() {
            Some("round") => transcript.push_str(&format!("\n== {text} ==\n")),
            Some("roll") => transcript.push_str(&format!(
                "    [{}] {}\n",
                entry["roll"]["id"].as_str().unwrap_or("?"),
                text
            )),
            _ => transcript.push_str(&format!("  {text}\n")),
        }
    }
    response
}
