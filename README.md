# Infinite-Deck Poker on VRFs

Provably fair poker where **no player — and no server — controls the randomness**, built on sr25519 VRFs ([schnorrkel](https://github.com/w3f/schnorrkel), the primitive Polkadot uses).

**▶ [Play it in your browser](https://emilbob.github.io/Infinite-deck-poker-using-VRFs/)** — the Rust engine is compiled to WebAssembly, so dealing *and* verification happen in your tab. There is no server and no API call.

The point is not the poker. It's a **dealer you don't have to trust**: every game emits a transcript any third party can re-verify, and tampering with any byte of it fails.

## What you can actually do

The page ships two modes:

- **Verify** — deal a hand, then edit the transcript by hand and watch `verify_transcript` reject it, in-browser, live.
- **Catch the Cheat** — ten rounds, some honest and some corrupted; you judge each one *before* the verifier adjudicates. The cheats are tiered by how a human could catch them — [`ByEye`, `ByArithmetic`, `Impossible`](Poker_VRF/src/cheats.rs) — so you clear the easy tier and then discover that you fundamentally cannot spot a forged VRF proof. That's the argument for the cryptography, delivered as an experience rather than a paragraph. Design note: [`docs/m3-catch-the-cheat.md`](docs/m3-catch-the-cheat.md).

The round's verdict is commit-revealed and withheld inside `api::Session` — it never crosses the wasm boundary until you submit an answer, so the game provably cannot change its mind after you guess.

## The protocol

```
commit    every player publishes  c_i = H(domain ‖ pubkey_i ‖ r_i)   (r_i secret)
reveal    every player opens r_i; all check H against c_i
seed      S = H(domain ‖ all c ‖ all r)                      (nobody steered it)
draw      every player VRF-signs S → (pre-output, proof)
output    o_i = VRFInOut::make_bytes(ctx)             (2Hash-DH, binds input+output)
cards     o_i → SHA-256 chain → 5 cards, rejection-sampled       (no modulo bias)
winner    best 5-card hand; ties broken by output bytes
```

Cards come from `VRFInOut::make_bytes` — the 2Hash-DH construction, which commits to the VRF *input* as well as the output — rather than from the raw `VRFPreOut` bytes, which are only a compressed group element and don't bind the input. `verify_draw` returns that output, so a hand is computable only *after* its proof has been checked.

Key properties:
- **Unpredictable** before the last reveal — the seed mixes *every* player's secret, so no participant can bias their own draw (each player signing their *own* commitment was the flaw in the original PoC; this fixes it).
- **Publicly verifiable** after — the full game is a `Transcript` any third party re-checks with `verify_transcript`: commitments, seed derivation, every VRF proof, every hand, the winner. Tampering with any byte fails verification (covered by tests).
- **Unbiased** — the "no modulo bias" claim above isn't decorative: a chi-square test asserts the sampler is uniform across all 52 cards, and a companion test feeds the *same* threshold a deliberately biased sampler to prove the test can actually detect bias rather than passing everything.
- **Infinite deck** — every card is an independent uniform draw, so duplicates are legal and *five of a kind* is the best hand in the game.
- **Portable** — a `Transcript` serializes to versioned JSON, so verification isn't confined to the process that played the game.

## Transcripts

`Transcript::to_json` emits a versioned document with every byte string hex-encoded; `Transcript::from_json` reads it back. The demo verifies *only what came off the wire* — it serializes, discards the in-memory game, and re-checks the decoded document.

```json
{
  "version": 1,
  "pubkeys":     ["<64 hex chars>", …],
  "commitments": ["<64 hex chars>", …],
  "reveals":     ["<64 hex chars>", …],
  "preouts":     ["<64 hex chars>", …],
  "proofs":      ["<128 hex chars>", …],
  "winner": 1
}
```

Decoding checks only well-formedness and field lengths — it says nothing about whether the game was honest. `verify_transcript` is what establishes that.

The encoding is deliberately **not byte-canonical**: verification re-derives everything from decoded fields and never reads the document text, so reformatting a transcript cannot change whether it verifies. The flip side is that the serialized bytes are *not* a transcript identity — don't hash the document as a commitment. A canonical digest over decoded fields would be a separate construction.

## Run it

The browser demo (the real surface):

```bash
# The wasm must exist first: web/package.json depends on ../Poker_VRF/pkg
# through a file: reference, which npm cannot resolve until wasm-pack creates it.
cd Poker_VRF && wasm-pack build --target web --out-dir pkg --release
cd ../web && npm ci && npm run dev
```

The engine on its own:

```bash
cd Poker_VRF
cargo run        # play a 3-player game + third-party verification, in the terminal
cargo test       # protocol, hand evaluation, sampler uniformity, tamper detection
```

`cargo run` puts you in seat 1 and asks for a passphrase — the web page has the same field. That passphrase becomes your secret contribution `r_1` to the shared seed; leave it blank and the system RNG supplies it.

This is the **only** place a human can act, and that's a property of the protocol rather than a missing feature: your hand is `f(seed, your_key)`, fixed the instant the seed exists. There is no draw or discard, because anything that let you change your cards after the fact would destroy the verifiability the whole design exists for.

Change one character of your passphrase and the entire deal changes; that's the seed doing its job (there's a test for it).

## Layout

```
Poker_VRF/
  src/lib.rs      protocol, cards, hand evaluator, Transcript + verify_transcript
  src/cheats.rs   deliberately corrupted games for Catch the Cheat, tiered by catchability
  src/api.rs      JSON-in/JSON-out façade + Session — plain Rust, tested natively
  src/wasm.rs     #[wasm_bindgen] shim — logic-free by design
  src/main.rs     terminal protocol demo (not a game surface)
web/
  src/engine.ts   typed wrapper over the wasm boundary; nothing else imports 'Poker_VRF'
  src/components/ Table, PlayingCard, TranscriptPanel, CatchTheCheat, SeedFingerprint
docs/             the Catch the Cheat design note, and the mental-poker no-go
```

All wasm traffic is JSON strings, reusing the transcript wire format — one encoding for the browser, the disk, and any third-party verifier. Logic lives in `api.rs` rather than `wasm.rs`, which is what lets the exact payloads the UI parses be tested natively, with no browser in the loop.

## Honest limitations

- Players are simulated in one process (or one tab); a real deployment needs a transport and a timeout/slashing story for players who commit but refuse to reveal — a griefing vector inherent to commit-reveal, and one this engine currently cannot even *express*. Concretely: because one process holds every secret, whoever reveals last could steer the seed. So this is a demo *of* the protocol, not a fair game under it. Both the binary and the page say so.
- **The infinite deck is a limitation dressed as a feature.** Every card being an i.i.d. draw is what makes duplicates legal and five of a kind the top hand — but it also means no card removal and no shared information, which is where real poker's depth comes from. It sidesteps *mental poker* (finite shared deck, no duplicates, hidden hole cards), which is the genuinely hard problem here.
- `secret_from_passphrase` is a domain-separated hash, not a password KDF. A guessable passphrase means a guessable contribution — survivable, since the seed stays unpredictable as long as any participant contributed real randomness, but it weakens *your* share.
- It deals one hand per player and picks a winner. There are no betting rounds, and that is a **decision, not a gap**: wagering on five cards you cannot change is a bet on a lottery ticket with extra steps. See the roadmap for why it was cut twice.
- `schnorrkel` is pinned to the 0.11 line: its VRF API is version-sensitive.
- v0.3.0 changed how cards are derived (pre-output → `make_bytes`), so the same keys and seed deal a different hand than v0.2.0. Transcripts don't cross that boundary — nothing had been persisted yet, which is why the change was made then.

## Status

Feature-complete as a demonstration. [`ROADMAP.md`](ROADMAP.md) tracks M1–M4, all closed: the verifiable engine, WASM + web demo, Catch the Cheat, and a mental-poker research spike that closed as a documented **no-go** ([`docs/m4-mental-poker.md`](docs/m4-mental-poker.md)) — no maintained Rust crate exists for Barnett–Smart-style shuffle proofs with threshold ElGamal over Ristretto, and the protocol shape is sequential in the player count rather than this engine's one parallel round. That note names the one thing worth revisiting if ever: a 2-player SRA toy that implements the quadratic-residuosity attack against itself, in the spirit of Catch the Cheat.
