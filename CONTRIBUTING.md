# Contributing

Upstream ([MadsWedendahlKruse/nat20](https://github.com/MadsWedendahlKruse/nat20)) does not accept
AI-generated contributions. If you have AI-assisted changes to Nat20's core, send them here
instead. Please don't report this fork's behaviour upstream.

## Before you start

- Changes land when they fit what [nat20-table](https://github.com/StGerman/nat20-table) needs.
  This fork does not take issues, so describe the problem in your pull request.
- Rules follow the 2024 rules as written in SRD 5.2.1. Older SRD data may fill a gap, never
  override a 2024 rule.
- Rules content comes only from the SRD (CC-BY-4.0) or your own words. Never copy text from
  published books or D&D Beyond. No strong-copyleft code or data.

## Pull requests

1. Branch from `demo` as `core/<topic>` and open the pull request against `demo`. One concern per
   pull request.
2. Follow upstream's patterns: registry JSON over code, Lua only where a rule needs logic, a test
   in `core/tests/`, and regenerated schemas in `assets/registries/.schemas`.
3. Run `cargo test --locked -p nat20_core` and `cargo deny check` before you push. `ci-ok` must be
   green.
4. Describe the change as **Before / After / How**, and say which parts an AI tool wrote.
5. Pull requests merge with a merge commit. Never rebase or force-push `demo`.

By contributing you agree that your work is licensed under the MIT licence in [`LICENSE`](LICENSE).
