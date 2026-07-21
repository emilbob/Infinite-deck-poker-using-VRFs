# Roadmap

**Verdict: finish.** The VRF core is now a real, verifiable game engine (v0.2.0). Remaining milestones turn it into something playable and showable.

The project autopilot (weekly, Wednesdays) builds the first unchecked milestone as a `milestone/*` PR; merging the PR advances the roadmap. Emil steers by editing this file or commenting on PRs.

## Milestones
- [x] M1: Verifiable game engine — shared-seed commit-reveal (fixes the sign-your-own-commitment flaw), VRF→cards with unbiased sampling, infinite-deck hand evaluator (five-of-a-kind top), `Transcript` + `verify_transcript` with tamper-detection tests, lib/bin split, clippy/fmt clean, `target/` untracked. Built live 2026-07-21.
- [ ] M2: Playable CLI game — multiple rounds with per-round seeds, simple betting (chips, check/bet/fold) on top of the verified core, and a transcript log written to disk; acceptance: a full 3-player match runs in the terminal and its saved transcript re-verifies.
- [ ] M3: WASM + web demo — compile the engine to wasm, minimal Vite page (Emil's standard stack) where a game plays out with the verification shown live; acceptance: `verify_transcript` runs in-browser on a game just played.
