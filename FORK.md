# About this fork

StGerman/nat20 is a permanent fork of [Nat20](https://github.com/MadsWedendahlKruse/nat20) by
Mads Wedendahl Kruse. It exists to serve [nat20-table](https://github.com/StGerman/nat20-table),
which wraps `nat20_core` in a JSON-lines API for an AI-run D&D 5e table (2024 rules, SRD 5.2.1).
Nothing here is proposed upstream, and upstream is not asked to support it.

Most changes in this fork are written with an AI coding assistant (Claude Code) and reviewed by
the fork's owner.

## Branches

| Branch | What it is | How it moves |
|---|---|---|
| `main` | An exact mirror of upstream `main` | Fast-forwarded to upstream, never committed to directly |
| `demo` (default) | Upstream `main` plus our core changes | Upstream `main` is merged in by `core/sync-upstream` pull requests; our pull requests merge with a merge commit. Never rebased or force-pushed |

nat20-table depends on `nat20_core` as a git dependency pinned by commit (`rev`) on `demo`, which
is why `demo`'s history is never rewritten.

We take nothing from upstream branches other than `main`. Work on an unmerged upstream branch
reaches this fork only after it lands on upstream `main` and the next sync merges it.

## What `demo` adds

Rules and content, all from SRD 5.2.1:

- Monster stat blocks as a registry kind (`assets/registries/monsters/`): Goblin Warrior, Goblin
  Boss, Wolf and Giant Rat, with fractional challenge ratings and SRD XP.
- Monster traits: Pack Tactics, Multiattack, Nimble Escape, a Bite that knocks Prone, and the
  Goblin Warrior's extra 1d4 on an attack with Advantage.
- The Prone condition. Advantage and Disadvantage cancel out whatever the count.
- The Cleric (levels 1 and 2), the Human species with Savage Attacker and Skilled, SRD
  backgrounds, Dragonborn Breath Weapon, Shield and Mace, and Potion of Healing.
- The Help action (assist a check, assist an attack, Stabilize), listed in
  [`docs/PROGRESS.md`](docs/PROGRESS.md) under Rules Glossary.
- Short rests spend Hit Point Dice; Second Wind and Rage regain one use on a Short Rest.
- Natural 20 and 1 decide only attack rolls and death saves; a natural 20 on a death save restores
  1 Hit Point; damage to a Stable creature is a failed death save.
- Unarmoured Armor Class counts the Dexterity modifier; heavy armour ignores a negative one.
- Surprised creatures roll Initiative with Disadvantage; concentration ends when the spell's
  effects run out; effects tied to a turn also end outside a fight.
- A walk into range ends on a free spot, never inside another creature's space.

Engine plumbing:

- `NAT20_REGISTRY_ROOT` overrides the folder registries load from (upstream fixes it at build time).
- Seedable dice, so a session replays exactly from a seed.
- One event sequence number across both logs.
- `nat20_core` re-exports `hecs`, `parry3d`, `rerecast` and `uom` (`nat20_core::hecs`, ...), so
  consumers use exactly the versions core was built with.

The full list is the [merged pull requests into `demo`](https://github.com/StGerman/nat20/pulls?q=is%3Apr+is%3Amerged+base%3Ademo).

## Upstream's README, read against `demo`

- [`docs/PROGRESS.md`](docs/PROGRESS.md) is not a comprehensive overview: it lists classes,
  spells and (ours) the Rules Glossary; species, items, conditions and monsters have no section yet.
- "No recompilation required" holds for JSON edits, but upstream compiles in the registry path;
  here, `NAT20_REGISTRY_ROOT` points the engine at another tree.

## Contributing

Upstream does not accept AI-generated contributions. If you want to contribute AI-assisted code
to Nat20, send it here instead: open a pull request against `demo`. Please:

- say in the description which parts an AI tool wrote;
- keep to upstream's patterns: registry JSON over code, a test in `core/tests/`, and regenerated
  `assets/registries/.schemas`;
- keep `cargo test --locked -p nat20_core` and `cargo deny check` green (the `ci-ok` check);
- use only SRD 5.1/5.2 (CC-BY-4.0) or your own text for rules content, never text from
  published books or D&D Beyond.

One concern per pull request. Changes land only if they fit what nat20-table needs; this fork does
not take issues. Please don't report fork-specific behaviour upstream.

## Checks

`demo` runs its own CI ([`.github/workflows/demo.yml`](.github/workflows/demo.yml)):
`cargo test --locked` and `cargo deny check` for `nat20_core` only. The GUI (`nat20_gui`) is
upstream's; it is not maintained or licence-checked here. Upstream's `rust.yml` is left as he wrote
it. Dependabot keeps dependencies current on `demo`.

## Licence

Code: MIT, as upstream (see [`LICENSE`](LICENSE)); our changes are under the same licence.
Rules content: SRD 5.2.1 under CC-BY-4.0; the attribution in the README's
[Legal (SRD)](README.md#legal-srd) section covers our additions too.
