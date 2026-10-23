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

- **Rules and content** from SRD 5.2.1 that the table needs and upstream does not have yet: monster
  stat blocks as a registry kind, more classes, species, feats, items, conditions and actions, and
  corrections where engine behaviour differed from the 2024 rules.
- **Engine plumbing** for a headless consumer: a registry root set at run time
  (`NAT20_REGISTRY_ROOT`), seedable dice so a session replays from a seed, a single event
  sequence, and re-exports of the crates whose types cross core's API (`nat20_core::<crate>`), so
  consumers build against exactly the versions core uses.

The record of what changed is the
[merged pull requests into `demo`](https://github.com/StGerman/nat20/pulls?q=is%3Apr+is%3Amerged+base%3Ademo);
[`docs/PROGRESS.md`](docs/PROGRESS.md) lists the implemented features it covers.

## Upstream's README, read against `demo`

- [`docs/PROGRESS.md`](docs/PROGRESS.md) is not a comprehensive overview of the engine; several
  registry kinds have no section in it.
- "No recompilation required" holds for JSON edits, but upstream compiles in the registry path;
  here, `NAT20_REGISTRY_ROOT` points the engine at another tree.

## Checks

`demo` runs its own CI ([`.github/workflows/demo.yml`](.github/workflows/demo.yml)) on
`nat20_core` only; `ci-ok` is the check a pull request needs. The GUI (`nat20_gui`) is upstream's;
it is not maintained or licence-checked here. Upstream's own workflow is left as he wrote it.
Dependabot keeps dependencies current on `demo`.

## Contributing

See [`CONTRIBUTING.md`](CONTRIBUTING.md).

## Licence

Code: MIT, as upstream (see [`LICENSE`](LICENSE)); our changes are under the same licence.
Rules content: SRD 5.2.1 under CC-BY-4.0; the attribution in the README's
[Legal (SRD)](README.md#legal-srd) section covers our additions too.
