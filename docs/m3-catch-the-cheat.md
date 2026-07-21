# M3 — Catch the Cheat

A game mode where the player, not the verifier, has to spot the tampering.

## The core constraint

`verify_transcript` catches everything instantly. That is the whole point of the
engine, and it is also why "spot the cheat" only becomes a game if the player is
put *in front of* the verifier:

```
1. deal a round — sometimes honest, sometimes tampered
2. the player judges: honest, or tampered?
3. only then does the verifier adjudicate
4. score by accuracy
```

The game is the player's judgement racing the cryptography.

## Why this is worth building

The cheats sort into tiers by *how* they are detectable, and that gradient is the
payload:

| Tier | Cheat | How a player could catch it |
| --- | --- | --- |
| 1 | Winner declared who does not hold the best hand | **By eye.** Read the table and compare hands. |
| 2 | A reveal that does not match its commitment | **By arithmetic.** Requires hashing — simple in principle, not doable by inspection. |
| 3 | A forged VRF proof | **Not at all.** No amount of staring gets there. |

Players clear Tier 1 confidently, then hit Tier 3 and discover they fundamentally
cannot. That is the argument for the cryptography, delivered as an experience
rather than a paragraph — and it explains the project better than the header
copy does.

## The game must not be trusted either

Before the player answers, the round shows a commitment:

```
round commitment  9f2a…c41d      ← H(verdict ‖ nonce)
```

After the answer, it reveals the verdict and the nonce so the player can check
the hash. The game therefore **cannot change its answer based on the guess**, and
it proves that using the same commit-reveal primitive the poker uses.

This is not decoration. It is a second, self-evident demonstration of the idea
the project exists to show, and it costs almost nothing given `commitment_hash`
already exists.

## Engine work

New `cheats.rs`:

```rust
pub enum Cheat {
    None,
    SwappedWinner,     // -> Error::WrongWinner
    TamperedReveal,    // -> Error::CommitmentMismatch
    StolenCommitment,  // -> Error::CommitmentMismatch (pubkey binding)
    ForgedProof,       // -> Error::BadVrfProof
    SwappedPreouts,    // -> Error::BadVrfProof
}

pub fn deal_round(players: usize, cheat: Cheat) -> Round
```

`StolenCommitment` is worth including specifically: it is the attack the existing
`commitment_is_bound_to_pubkey` test covers, and it is the subtlest of the Tier-2
set — a commitment that is perfectly well-formed, just not *yours*.

### The one place this must not be sloppy

**Every variant needs a test asserting `verify_transcript` rejects it with the
expected error, and that `Cheat::None` verifies.** Without those, the game can
present a "cheat" that is actually valid, or call an honest round tampered — and
a game about detecting dishonesty that is itself wrong about the answer is worse
than not shipping it.

## Difficulty ladder

- **Rounds 1–3** — Tier 1 only. Careful reading wins.
- **Rounds 4–6** — Tiers 1 and 2 mixed. Eyeballing starts to fail.
- **Rounds 7–10** — all tiers, honest rounds included. The lesson lands.

Optional hard mode: name *which* check fails, not merely whether one does.

## UI

Reuses the existing table and transcript panel. The tamper buttons are replaced
by a verdict bar:

```
ROUND 04 / 10        COMMITMENT 9f2a…c41d        SCORE 3/3

          [  HONEST  ]         [  TAMPERED  ]

── after answering ────────────────────────────────
✗ WRONG — this round was tampered
  Forged VRF proof on player 2.
  Nothing visible in the cards could have told you.
  Only the proof check catches this.
```

That closing line is what makes a wrong answer read as a lesson rather than a
gotcha, and it is where the tier the cheat belongs to gets stated explicitly.

## Effort

- **Rust** — ~150 lines plus a test per variant. Small: the verifier already
  produces exactly the right errors, so the work is constructing genuinely
  invalid transcripts rather than detecting them.
- **API** — one function through the existing JSON boundary.
- **UI** — a mode toggle plus round/score components. The bulk of the work, and
  still roughly a day.

## Why this before betting

Betting adds a thin game to a strong demo. With no draws and no community cards,
wagering on five fixed cards is closer to betting on a lottery ticket than to
poker — the strategic depth of real poker comes from card removal and shared
information, neither of which an infinite deck has.

Catch the Cheat instead turns the project's actual differentiator into the
gameplay: a stranger can catch you cheating, in their own browser, with no
server involved.
