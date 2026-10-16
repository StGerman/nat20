# nat20_server

A headless JSON API around `nat20_core`, so something other than the debug GUI
(a DM agent, an AI player, a test script) can run a fight. One JSON request per
line on stdin, one JSON response per line on stdout.

```bash
cargo run -p nat20_server                  # the API over stdio
cargo run -p nat20_server --bin nat20_duel # Fighter vs Goblin Warrior, transcript + audit
cargo test -p nat20_server
```

## Requests

| `cmd` | Fields | What it does |
|---|---|---|
| `load_scene` | `scene_id` (file in `data/scenes`) or inline `scene` | Spawns the creatures, rolls initiative, starts combat, runs scripted creatures until a client decision is needed. |
| `get_state` | | Round, whose turn, initiative, and per creature: HP, AC, life state, position, distances in feet. |
| `legal_actions` | `actor` | Every action the creature can take now: its options (`context`, e.g. which weapon), cost, and valid targets with `reach` (`in_range` or `move_then_act`). Also `unavailable` actions with the engine's reason. |
| `submit` | `actor`, `action`, `context` (default 0), `targets` (handles) | Validates and resolves the action. Self-targeted actions need no target. |
| `react` | `actor`, `choice` (index or `null` to decline) | Answers a reaction prompt. |
| `end_turn` | `actor` | Ends the turn. |
| `events` | `since` | The log from entry `since`. |

Every response carries `ok`, `result` or `error` (`code`, `message`),
`new_events` (log entries this request produced) and `awaiting` (who the engine
is waiting on: `action`, `reaction` with options, `combat_over` with the
winner). A rejected request changes nothing.

## The log and the roll audit

Every die the engine rolls becomes a log entry with a `roll` object: id
(`r<seq>`), kind (`initiative`, `attack`, `saving_throw`, `skill_check`,
`damage`, `healing`), each die, each modifier with its source, the total, the
AC or DC it was against and the outcome. Initiative is read from the encounter
because Nat20 does not emit an event for it.

`audit::audit_transcript` checks a transcript against the log: every cited
roll (`[r12] ... = 19 vs AC 15`) must exist with the same total and AC/DC,
every line that shows roll arithmetic must cite a roll, and every logged roll
must be cited. `audit::check_roll_arithmetic` checks each logged roll adds up.

## Scripted creatures

Creatures with `"control": "scripted"` are run by the server: attack the
nearest hostile creature, preferring a melee weapon already in reach, then any
weapon in range, then moving into melee; end the turn when nothing is possible;
always take the first reaction offered. They carry `PlayerControlledTag` so the
engine's built-in random AI never acts for them.

## Content

- `data/monsters/goblin_warrior.json`: the SRD 5.2.1 Goblin Warrior. Spawning
  fails if the engine's AC differs from the stat block. `not_yet_supported`
  lists what the engine cannot do yet (Shield as equipment, the extra 1d4 on
  Advantage, Nimble Escape, Stealth +6, fractional CR).
- `pregen.rs`: Bren, a level 2 Fighter (Defense style, Chain Mail, Longsword).
  Dragonborn and no Shield until Human and shields exist in the engine.
- `data/scenes/duel.json`: Bren and the goblin, 5 ft apart.

## Not in this crate yet

Free movement (only `move_then_act` through an attack), `request_check`,
`short_rest`, named anchor points, spells with area targeting, and variant
actions.
