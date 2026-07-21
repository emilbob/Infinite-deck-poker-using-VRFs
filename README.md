# Infinite-Deck Poker on VRFs

Provably fair poker where **no player — and no server — controls the randomness**, built on sr25519 VRFs ([schnorrkel](https://github.com/w3f/schnorrkel), the primitive Polkadot uses).

## The protocol

```
commit    every player publishes  c_i = H(pubkey_i ‖ r_i)     (r_i secret)
reveal    every player opens r_i; all check H against c_i
seed      S = H(all commitments ‖ all reveals)                (nobody steered it)
draw      every player VRF-signs S → (pre-output, proof)
output    o_i = make_bytes(VRFInOut)                          (2Hash-DH, binds input+output)
cards     o_i → SHA-256 chain → 5 cards, rejection-sampled    (no modulo bias)
winner    best 5-card hand; ties broken by output bytes
```

Cards come from `VRFInOut::make_bytes` — the 2Hash-DH construction, which commits to the VRF *input* as well as the output — rather than from the raw `VRFPreOut` bytes, which are only a compressed group element and don't bind the input. `verify_draw` returns that output, so a hand is computable only *after* its proof has been checked.

Key properties:
- **Unpredictable** before the last reveal — the seed mixes *every* player's secret, so no participant can bias their own draw (each player signing their *own* commitment was the flaw in the original PoC; this fixes it).
- **Publicly verifiable** after — the full game is a `Transcript` any third party re-checks with `verify_transcript`: commitments, seed derivation, every VRF proof, every hand, the winner. Tampering with any byte fails verification (covered by tests).
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

```bash
cd Poker_VRF
cargo run        # play a 3-player game + third-party verification
cargo test       # protocol, hand-evaluation, and tamper-detection tests
```

`cargo run` puts you in seat 1 and asks for a passphrase. That passphrase becomes your secret contribution `r_1` to the shared seed — press enter instead and the system RNG supplies it.

This is the **only** place a human can act, and that is a property of the protocol rather than a missing feature: your hand is `f(seed, your_key)`, fixed the instant the seed exists. There is no draw or discard, because anything that let you change your cards after the fact would destroy the verifiability the whole design exists for. Betting — deciding what to do about a hand you can see but cannot change — is where real gameplay lives, and it's M2 on the roadmap.

Change one character of your passphrase and the entire deal changes; that's the seed doing its job (there's a test for it).

## Layout

```
Poker_VRF/src/lib.rs    protocol, cards, hand evaluator, transcript verification
Poker_VRF/src/main.rs   demo game
```

## Honest limitations

- Players are simulated in one process; a real deployment needs a transport and a timeout/slashing story for players who commit but refuse to reveal (a griefing vector inherent to commit-reveal). Concretely: because one process holds every secret, whoever reveals last could steer the seed — so the CLI is a demo *of* the protocol, not a fair game under it. The binary says so on exit.
- `secret_from_passphrase` is a domain-separated hash, not a password KDF. A guessable passphrase means a guessable contribution — survivable, since the seed stays unpredictable as long as any participant contributed real randomness, but it weakens *your* share.
- It deals one hand per player and picks a winner — betting rounds and game flow are roadmap items, not present.
- `schnorrkel` is pinned to the 0.11 line: its VRF API is version-sensitive.
- v0.3.0 changed how cards are derived (pre-output → `make_bytes`), so the same keys and seed now deal a different hand than v0.2.0. Transcripts don't cross that boundary — nothing had been persisted yet, which is why the change was made now.
