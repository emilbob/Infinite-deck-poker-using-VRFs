# Roadmap

**Verdict: finish.** The VRF core is now a real, verifiable game engine (v0.2.0). Remaining milestones turn it into something playable and showable.

The project autopilot (weekly, Wednesdays) builds the first unchecked milestone as a `milestone/*` PR; merging the PR advances the roadmap. Emil steers by editing this file or commenting on PRs.

## Milestones
- [x] M1: Verifiable game engine — shared-seed commit-reveal (fixes the sign-your-own-commitment flaw), VRF→cards with unbiased sampling, infinite-deck hand evaluator (five-of-a-kind top), `Transcript` + `verify_transcript` with tamper-detection tests, lib/bin split, clippy/fmt clean, `target/` untracked. Built live 2026-07-21.
- [x] M2: WASM + web demo — compile the engine to wasm, Vite + React 19 + Tailwind v4 page (no r3f/Lenis: no WebGL scene, no scroll narrative) where a game plays out with verification shown live, including a tamper panel that edits a transcript byte in-browser and shows the resulting error. Acceptance: `verify_transcript` runs in-browser on a game just played, and a tampered byte visibly fails. Built 2026-07-21; published to Pages by `.github/workflows/web.yml`.
- [ ] M3: Betting in the browser — multiple rounds with per-round seeds, chips and check/bet/fold on top of the verified core, transcripts downloadable per round.

> **2026-07-21 — reordered.** The original M2 was a betting CLI. Dropped: it was throwaway UI that would be rebuilt in the browser immediately, so betting moved into the web milestone instead. The terminal binary stays a protocol demo, not a game surface.
