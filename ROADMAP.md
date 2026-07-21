# Roadmap

**Verdict: finish.** The VRF core is now a real, verifiable game engine (v0.2.0). Remaining milestones turn it into something playable and showable.

The project autopilot (weekly, Wednesdays) builds the first unchecked milestone as a `milestone/*` PR; merging the PR advances the roadmap. Emil steers by editing this file or commenting on PRs.

## Milestones
- [x] M1: Verifiable game engine — shared-seed commit-reveal (fixes the sign-your-own-commitment flaw), VRF→cards with unbiased sampling, infinite-deck hand evaluator (five-of-a-kind top), `Transcript` + `verify_transcript` with tamper-detection tests, lib/bin split, clippy/fmt clean, `target/` untracked. Built live 2026-07-21.
- [x] M2: WASM + web demo — compile the engine to wasm, Vite + React 19 + Tailwind v4 page (no r3f/Lenis: no WebGL scene, no scroll narrative) where a game plays out with verification shown live, including a tamper panel that edits a transcript byte in-browser and shows the resulting error. Acceptance: `verify_transcript` runs in-browser on a game just played, and a tampered byte visibly fails. Built 2026-07-21; published to Pages by `.github/workflows/web.yml`.
- [x] M3: **Catch the Cheat** — a game mode where the engine deals rounds that are sometimes honest and sometimes tampered, and the player judges each one *before* the verifier adjudicates. See [`docs/m3-catch-the-cheat.md`](docs/m3-catch-the-cheat.md). Acceptance: ten rounds play through with a score; every `Cheat` variant has a test proving `verify_transcript` rejects it with the expected error, and that `Cheat::None` verifies; the round's verdict is commit-revealed so the game provably cannot change its answer after the player guesses. Built 2026-07-21 (#9). The verdict is withheld inside `api::Session` and never crosses the wasm boundary until an answer is submitted, so the commitment is a promise rather than a prop.
- [ ] M4: Betting in the browser — multiple rounds with per-round seeds, chips and check/bet/fold on top of the verified core, transcripts downloadable per round. Requires hidden hands: players withhold their `(pre-output, proof)` until showdown, which works because the VRF output is deterministic and the proof still verifies against the seed afterwards.

> **2026-07-21 — reordered (1).** The original M2 was a betting CLI. Dropped: it was throwaway UI that would be rebuilt in the browser immediately, so betting moved into the web milestone instead. The terminal binary stays a protocol demo, not a game surface.
>
> **2026-07-21 — reordered (2).** Betting moved from M3 to M4, behind Catch the Cheat. Betting adds a thin game to a strong demo: with no draws and no community cards, wagering on five fixed cards is closer to a bet on a lottery ticket than to poker. Catch the Cheat instead makes the existing demo explain itself, is less work, and turns the project's actual differentiator — that a stranger can catch you cheating in their own browser — into the gameplay.
