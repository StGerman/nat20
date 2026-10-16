//! Milestone 1: one Fighter against one Goblin Warrior, end to end through the
//! JSON API.

use std::{
    io::Write,
    process::{Command, Stdio},
};

use nat20_server::{
    audit::{audit_transcript, check_roll_arithmetic},
    duel::run_duel,
    session::Session,
};
use serde_json::{Value, json};

fn call(session: &mut Session, request: Value) -> Value {
    serde_json::from_str(&session.handle_json(&request.to_string())).unwrap()
}

fn creature<'a>(state: &'a Value, handle: &str) -> &'a Value {
    state["result"]["creatures"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["handle"] == handle)
        .unwrap()
}

#[test]
fn duel_finishes_and_every_roll_is_in_the_engine_log() {
    // Dice are random, so play enough duels to cover hits, misses, crits,
    // Second Wind and both sides winning at least sometimes.
    for run in 0..25 {
        let (transcript, log) = run_duel();

        assert!(
            log.iter().any(|e| e.kind == "combat_over"),
            "run {run}: combat never ended\n{transcript}"
        );
        let rolls: Vec<_> = log.iter().filter_map(|e| e.roll.as_ref()).collect();
        assert!(
            rolls.iter().filter(|r| r.kind == "initiative").count() == 2,
            "run {run}: expected two initiative rolls\n{transcript}"
        );
        assert!(
            rolls.iter().any(|r| r.kind == "attack"),
            "run {run}: no attack rolled\n{transcript}"
        );
        for roll in &rolls {
            check_roll_arithmetic(roll).unwrap_or_else(|e| panic!("run {run}: {e}\n{transcript}"));
        }

        let report = audit_transcript(&transcript, &log, true);
        assert!(
            report.passed(),
            "run {run}: audit failed: {:?}\n{transcript}",
            report.problems
        );
        assert_eq!(report.rolls_cited, report.rolls_in_log);
    }
}

#[test]
fn tampered_transcript_fails_the_audit() {
    let (transcript, log) = run_duel();
    let attack = log
        .iter()
        .filter_map(|e| e.roll.as_ref())
        .find(|r| r.kind == "attack")
        .expect("every duel has an attack");

    // An LLM "improving" a roll: same citation, different total.
    let line = transcript
        .lines()
        .find(|l| l.contains(&format!("[{}]", attack.id)))
        .unwrap();
    let forged_line = line.replace(
        &format!(" = {} ", attack.total),
        &format!(" = {} ", attack.total + 7),
    );
    assert_ne!(line, forged_line);
    let forged = transcript.replace(line, &forged_line);
    assert!(!audit_transcript(&forged, &log, true).passed());

    // An invented roll with no engine citation.
    let invented = format!("{transcript}\nThe goblin rolls a d20 and gets 19.\n");
    assert!(!audit_transcript(&invented, &log, true).passed());

    // A dropped roll.
    let dropped: String = transcript
        .lines()
        .filter(|l| !l.contains(&format!("[{}]", attack.id)))
        .map(|l| format!("{l}\n"))
        .collect();
    assert!(!audit_transcript(&dropped, &log, true).passed());
}

#[test]
fn goblin_warrior_matches_the_srd_stat_block() {
    let mut session = Session::default();
    let loaded = call(
        &mut session,
        json!({ "cmd": "load_scene", "scene_id": "duel" }),
    );
    assert_eq!(loaded["ok"], true, "{loaded}");

    let goblin = creature(&loaded, "goblin");
    assert_eq!(goblin["name"], "Goblin Warrior");
    assert_eq!(goblin["ac"], 15);
    assert_eq!(goblin["hp"], 10);
    assert_eq!(goblin["max_hp"], 10);

    let bren = creature(&loaded, "bren");
    assert_eq!(bren["ac"], 17, "Chain Mail 16 + Defense 1");
    assert_eq!(bren["max_hp"], 20, "level 2 Fighter, CON 14");

    // Play duels until the goblin has attacked and hit, then check its
    // Scimitar numbers: +4 to hit, 1d6+2 damage.
    for _ in 0..40 {
        let (_, log) = run_duel();
        let goblin_rolls: Vec<_> = log
            .iter()
            .filter(|e| e.actor.as_deref() == Some("goblin"))
            .filter_map(|e| e.roll.as_ref())
            .collect();
        let Some(attack) = goblin_rolls.iter().find(|r| r.kind == "attack") else {
            continue;
        };
        let to_hit: i32 = attack.modifiers.iter().map(|m| m.value).sum();
        assert_eq!(to_hit, 4, "Scimitar attack bonus: {attack:?}");

        let Some(damage) = goblin_rolls.iter().find(|r| r.kind == "damage") else {
            continue;
        };
        assert_eq!(
            damage.dice[0].dice,
            if damage.label.contains("critical") {
                "2d6"
            } else {
                "1d6"
            }
        );
        let flat: i32 = damage.modifiers.iter().map(|m| m.value).sum();
        assert_eq!(flat, 2, "Scimitar damage bonus: {damage:?}");
        return;
    }
    panic!("the goblin never hit in 40 duels");
}

#[test]
fn invalid_decisions_get_readable_rejections() {
    let mut session = Session::default();

    let early = call(&mut session, json!({ "cmd": "get_state" }));
    assert_eq!(early["error"]["code"], "no_scene");

    call(
        &mut session,
        json!({ "cmd": "load_scene", "scene_id": "duel" }),
    );
    // Scripted goblin turns run inside the session, so after loading the
    // engine always waits on Bren (or the fight is already over).
    let state = call(&mut session, json!({ "cmd": "get_state" }));
    if state["awaiting"]["kind"] != "action" {
        return;
    }
    assert_eq!(state["awaiting"]["actor"], "bren");

    let cases = [
        (
            json!({ "cmd": "submit", "actor": "goblin", "action": "nat20_core::action.melee_attack", "targets": ["bren"] }),
            "not_your_turn",
        ),
        (
            json!({ "cmd": "submit", "actor": "bren", "action": "nat20_core::action.fireball" }),
            "rejected",
        ),
        (
            json!({ "cmd": "submit", "actor": "bren", "action": "nat20_core::action.melee_attack" }),
            "rejected",
        ),
        (
            json!({ "cmd": "submit", "actor": "bren", "action": "nat20_core::action.melee_attack", "targets": ["dragon"] }),
            "unknown_creature",
        ),
        (
            json!({ "cmd": "react", "actor": "bren", "choice": 0 }),
            "not_awaiting_you",
        ),
        (
            json!({ "cmd": "end_turn", "actor": "goblin" }),
            "not_your_turn",
        ),
        (json!({ "cmd": "fly" }), "bad_request"),
    ];
    for (request, code) in cases {
        let response = call(&mut session, request.clone());
        assert_eq!(response["ok"], false, "{request} should fail: {response}");
        assert_eq!(response["error"]["code"], code, "{request}: {response}");
        assert!(!response["error"]["message"].as_str().unwrap().is_empty());
        // A rejection changes nothing.
        assert_eq!(response["new_events"], json!([]), "{request}");
        assert_eq!(response["awaiting"]["actor"], "bren");
    }
}

#[test]
fn legal_actions_list_the_longsword_attack_on_the_goblin() {
    let mut session = Session::default();
    let loaded = call(
        &mut session,
        json!({ "cmd": "load_scene", "scene_id": "duel" }),
    );
    if loaded["awaiting"]["kind"] != "action" {
        return; // the goblin won before Bren's first turn
    }
    let legal = call(
        &mut session,
        json!({ "cmd": "legal_actions", "actor": "bren" }),
    );
    let actions = legal["result"]["actions"].as_array().unwrap();
    let attack = actions
        .iter()
        .find(|a| a["action"] == "nat20_core::action.melee_attack")
        .unwrap_or_else(|| panic!("no melee attack in {legal}"));
    let option = &attack["options"][0];
    assert_eq!(option["weapon"], "longsword");
    assert_eq!(option["targets"][0]["handle"], "goblin");
    assert_eq!(option["targets"][0]["reach"], "in_range");
    assert_eq!(option["targets"][0]["attitude"], "hostile");
    for id in [
        "nat20_core::action.fighter.second_wind",
        "nat20_core::action.fighter.action_surge",
    ] {
        assert!(
            actions.iter().any(|a| a["action"] == id),
            "{id} missing from {legal}"
        );
    }
}

#[test]
fn stdio_binary_speaks_json_lines() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_nat20_server"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    {
        let stdin = child.stdin.as_mut().unwrap();
        writeln!(stdin, r#"{{"cmd":"load_scene","scene_id":"duel"}}"#).unwrap();
        writeln!(stdin, r#"{{"cmd":"events","since":0}}"#).unwrap();
    }
    let output = child.wait_with_output().unwrap();
    let lines: Vec<Value> = String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0]["ok"], true);
    let events = lines[1]["result"]["events"].as_array().unwrap();
    assert!(events.iter().any(|e| e["roll"]["kind"] == "initiative"));
}

#[test]
fn second_load_scene_returns_the_new_scenes_events() {
    let mut session = Session::default();
    let load = json!({ "cmd": "load_scene", "scene_id": "duel" });
    call(&mut session, load.clone());
    let second = call(&mut session, load);
    assert_eq!(second["ok"], true, "{second}");
    let events = second["new_events"].as_array().unwrap();
    assert_eq!(events[0]["seq"], 0, "{second}");
    assert_eq!(
        events
            .iter()
            .filter(|e| e["roll"]["kind"] == "initiative")
            .count(),
        2,
        "{second}"
    );
}

#[test]
fn bad_scenes_are_rejected_before_combat_starts() {
    let creature = |handle: &str, side: &str, x: f32| {
        json!({
            "handle": handle, "monster": "goblin_warrior", "side": side,
            "control": "scripted", "position_m": [x, 0.0, 0.0]
        })
    };
    let bren = json!({
        "handle": "bren", "pregen": "fighter_l2", "side": "party",
        "control": "client", "position_m": [0.0, 0.0, 0.0]
    });
    let cases = [
        json!([]),
        // Two goblins share a faction, so neither can ever attack the other.
        json!([creature("a", "red", 0.0), creature("b", "blue", 1.5)]),
        // Bren is hostile to the goblin, so they cannot be on one side.
        json!([bren, creature("goblin", "party", 1.5)]),
    ];
    for creatures in cases {
        let mut session = Session::default();
        let response = call(
            &mut session,
            json!({ "cmd": "load_scene", "scene": { "id": "bad", "creatures": creatures } }),
        );
        assert_eq!(response["ok"], false, "{creatures}: {response}");
        assert_eq!(response["error"]["code"], "bad_scene", "{response}");
        let state = call(&mut session, json!({ "cmd": "get_state" }));
        assert_eq!(state["error"]["code"], "no_scene");
    }
}

#[test]
fn monster_with_the_wrong_proficiency_bonus_is_rejected() {
    let dir = std::env::temp_dir().join(format!("nat20_pb_{}", std::process::id()));
    std::fs::create_dir_all(dir.join("monsters")).unwrap();
    let mut goblin: Value = serde_json::from_str(
        &std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("data/monsters/goblin_warrior.json"),
        )
        .unwrap(),
    )
    .unwrap();
    goblin["proficiency_bonus"] = json!(3);
    std::fs::write(dir.join("monsters/bad_goblin.json"), goblin.to_string()).unwrap();
    let mut session = Session::new(dir.clone());
    let response = call(
        &mut session,
        json!({ "cmd": "load_scene", "scene": { "id": "pb", "creatures": [{
            "handle": "goblin", "monster": "bad_goblin", "side": "monsters",
            "control": "scripted", "position_m": [0.0, 0.0, 0.0]
        }] } }),
    );
    assert_eq!(response["error"]["code"], "bad_monster", "{response}");
    assert!(
        response["error"]["message"]
            .as_str()
            .unwrap()
            .contains("proficiency_bonus"),
        "{response}"
    );
    std::fs::remove_dir_all(dir).ok();
}
