# Infinite-Deck Poker on VRFs

Provably fair poker where **no player — and no server — controls the randomness**, built on sr25519 VRFs ([schnorrkel](https://github.com/w3f/schnorrkel), the primitive Polkadot uses).

## The protocol

```
commit    every player publishes  c_i = H(pubkey_i ‖ r_i)     (r_i secret)
reveal    every player opens r_i; all check H against c_i
seed      S = H(all commitments ‖ all reveals)                (nobody steered it)
draw      every player VRF-signs S → (pre-output, proof)
cards     pre-output → SHA-256 chain → 5 cards, rejection-sampled (no modulo bias)
winner    best 5-card hand; ties broken by pre-output bytes
```

Key properties:
- **Unpredictable** before the last reveal — the seed mixes *every* player's secret, so no participant can bias their own draw (each player signing their *own* commitment was the flaw in the original PoC; this fixes it).
- **Publicly verifiable** after — the full game is a `Transcript` any third party re-checks with `verify_transcript`: commitments, seed derivation, every VRF proof, every hand, the winner. Tampering with any byte fails verification (covered by tests).
- **Infinite deck** — every card is an independent uniform draw, so duplicates are legal and *five of a kind* is the best hand in the game.

## Run it

```bash
cd Poker_VRF
cargo run        # play a 3-player game + third-party verification
cargo test       # protocol, hand-evaluation, and tamper-detection tests
```

## Layout

```
Poker_VRF/src/lib.rs    protocol, cards, hand evaluator, transcript verification
Poker_VRF/src/main.rs   demo game
```

## Honest limitations

- Players are simulated in one process; a real deployment needs a transport and a timeout/slashing story for players who commit but refuse to reveal (a griefing vector inherent to commit-reveal).
- It deals one hand per player and picks a winner — betting rounds and game flow are roadmap items, not present.
- `schnorrkel` is pinned to the 0.11 line: its VRF API is version-sensitive.
