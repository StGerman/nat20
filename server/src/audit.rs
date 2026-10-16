//! Checks a transcript against the engine log: the demo's rule that every
//! roll a player sees must have come from the engine.
//!
//! A transcript cites a roll as `[r<seq>]` followed by the roll's arithmetic,
//! e.g. `[r12] ... = 19 vs AC 15`. The audit fails when:
//! - a citation names a roll the log does not have,
//! - a cited total or AC/DC differs from the log,
//! - a line shows roll arithmetic (`d20`, `= <n>`) without a citation,
//! - (when `require_all`) a roll in the log is never cited.

use std::collections::{HashMap, HashSet};

use crate::log::{LogEntry, Roll};

#[derive(Debug, Default)]
pub struct AuditReport {
    pub rolls_in_log: usize,
    pub rolls_cited: usize,
    pub problems: Vec<String>,
}

impl AuditReport {
    pub fn passed(&self) -> bool {
        self.problems.is_empty()
    }
}

pub fn audit_transcript(transcript: &str, log: &[LogEntry], require_all: bool) -> AuditReport {
    let rolls: HashMap<&str, &Roll> = log
        .iter()
        .filter_map(|e| e.roll.as_ref())
        .map(|r| (r.id.as_str(), r))
        .collect();
    let mut cited = HashSet::new();
    let mut report = AuditReport {
        rolls_in_log: rolls.len(),
        ..Default::default()
    };

    for (line_no, line) in transcript.lines().enumerate() {
        let line_no = line_no + 1;
        let Some(id) = citation(line) else {
            if looks_like_roll(line) {
                report.problems.push(format!(
                    "line {line_no}: roll shown without an engine citation: {line}"
                ));
            }
            continue;
        };
        let Some(roll) = rolls.get(id.as_str()) else {
            report.problems.push(format!(
                "line {line_no}: cites {id}, which is not in the engine log"
            ));
            continue;
        };
        cited.insert(id.clone());
        match number_after(line, " = ") {
            Some(total) if total == roll.total => {}
            Some(total) => report.problems.push(format!(
                "line {line_no}: {id} total {total} but engine rolled {}",
                roll.total
            )),
            None => report
                .problems
                .push(format!("line {line_no}: {id} is cited without its total")),
        }
        for kind in ["AC", "DC"] {
            if let Some(value) = number_after(line, &format!(" vs {kind} ")) {
                match &roll.against {
                    Some(against) if against.kind == kind && against.value == value => {}
                    other => report.problems.push(format!(
                        "line {line_no}: {id} shows {kind} {value} but engine has {other:?}"
                    )),
                }
            }
        }
    }

    report.rolls_cited = cited.len();
    if require_all {
        let mut missing: Vec<&str> = rolls
            .keys()
            .copied()
            .filter(|id| !cited.contains(*id))
            .collect();
        missing.sort_by_key(|id| id[1..].parse::<usize>().unwrap_or(0));
        for id in missing {
            report
                .problems
                .push(format!("engine roll {id} never appears in the transcript"));
        }
    }
    report
}

/// Checks a logged roll is internally consistent: every die is within its
/// size, and the total is the dice that count plus the flat modifiers.
pub fn check_roll_arithmetic(roll: &Roll) -> Result<(), String> {
    let mut sum = 0i64;
    for die in &roll.dice {
        let (count, size) = die
            .dice
            .split_once('d')
            .and_then(|(n, s)| Some((n.parse::<usize>().ok()?, s.parse::<u32>().ok()?)))
            .ok_or_else(|| format!("{}: bad dice {}", roll.id, die.dice))?;
        if die.results.len() != count {
            return Err(format!(
                "{}: {} shows {} results",
                roll.id,
                die.dice,
                die.results.len()
            ));
        }
        if let Some(bad) = die.results.iter().find(|r| **r < 1 || **r > size) {
            return Err(format!("{}: {bad} is not a valid d{size} result", roll.id));
        }
        sum += match die.kept {
            Some(kept) if die.results.contains(&kept) => kept as i64,
            Some(kept) => return Err(format!("{}: kept {kept} was never rolled", roll.id)),
            None => die.results.iter().map(|r| *r as i64).sum(),
        };
    }
    sum += roll.modifiers.iter().map(|m| m.value as i64).sum::<i64>();
    if sum != roll.total as i64 {
        return Err(format!(
            "{}: dice and modifiers add to {sum}, total says {}",
            roll.id, roll.total
        ));
    }
    if let (Some(against), Some(outcome)) = (&roll.against, &roll.outcome) {
        let natural = roll
            .dice
            .first()
            .and_then(|d| d.kept.or(d.results.first().copied()));
        let expected_success = match (roll.kind.as_str(), natural) {
            ("attack", Some(20)) => true,
            ("attack", Some(1)) => false,
            _ => roll.total >= against.value,
        };
        let success = matches!(outcome.as_str(), "hit" | "critical hit" | "success");
        if success != expected_success {
            return Err(format!(
                "{}: {} vs {} {} is marked {outcome}",
                roll.id, roll.total, against.kind, against.value
            ));
        }
    }
    Ok(())
}

fn citation(line: &str) -> Option<String> {
    let start = line.find("[r")?;
    let rest = &line[start + 2..];
    let end = rest.find(']')?;
    let digits = &rest[..end];
    (!digits.is_empty() && digits.chars().all(|c| c.is_ascii_digit())).then(|| format!("r{digits}"))
}

fn number_after(line: &str, marker: &str) -> Option<i32> {
    let start = line.find(marker)? + marker.len();
    let rest = &line[start..];
    let end = rest
        .char_indices()
        .find(|(i, c)| !(c.is_ascii_digit() || (*i == 0 && *c == '-')))
        .map(|(i, _)| i)
        .unwrap_or(rest.len());
    rest[..end].parse().ok()
}

fn looks_like_roll(line: &str) -> bool {
    // API request echo lines carry JSON, not results.
    if line.trim_start().starts_with("api>") {
        return false;
    }
    line.contains("d20") || number_after(line, " = ").is_some()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::log::Against;

    fn log_with_roll() -> Vec<LogEntry> {
        vec![LogEntry {
            seq: 3,
            kind: "roll".into(),
            round: Some(1),
            actor: Some("bren".into()),
            target: Some("goblin".into()),
            text: String::new(),
            roll: Some(Roll {
                id: "r3".into(),
                kind: "attack".into(),
                label: "attack roll vs goblin".into(),
                dice: Vec::new(),
                modifiers: Vec::new(),
                total: 19,
                against: Some(Against {
                    kind: "AC".into(),
                    value: 15,
                }),
                outcome: Some("hit".into()),
            }),
        }]
    }

    #[test]
    fn accepts_matching_citation() {
        let report = audit_transcript(
            "[r3] Bren attacks: 1d20 (14) +5 = 19 vs AC 15, hit.",
            &log_with_roll(),
            true,
        );
        assert!(report.passed(), "{:?}", report.problems);
    }

    #[test]
    fn rejects_wrong_total_unknown_id_and_uncited_roll() {
        let transcript =
            "[r3] Bren attacks: = 20 vs AC 15\n[r9] = 4\nThe goblin rolls a d20 and gets 17.";
        let report = audit_transcript(transcript, &log_with_roll(), true);
        assert_eq!(report.problems.len(), 3, "{:?}", report.problems);
    }

    #[test]
    fn rejects_missing_roll_when_required() {
        let report = audit_transcript("Bren swings.", &log_with_roll(), true);
        assert!(!report.passed());
    }
}
