# M4 — Mental poker research spike: finite deck, go/no-go

**Recommendation up front: no-go, for this repo, right now.** Keep shipping the infinite
deck. Reasoning follows.

## The question

The current engine draws each player's hand i.i.d. from an infinite deck: no card
removal, no hidden information shared across players, no way for a card in your hand
to make a card in mine less likely. That's exactly the depth this milestone would buy
back — a real 52-card deck, no duplicates, hidden hole cards, dealt with no trusted
dealer. This is the actual "mental poker" problem from the 1979 Shamir–Rivest–Adleman
paper, and it is a different and much harder problem than what M1–M3 solved.

## Candidate protocols

### SRA — commutative encryption (Shamir, Rivest, Adleman, 1979/1981, *[Mental
Poker](https://people.csail.mit.edu/rivest/pubs/SRA81.pdf)*)

Each player encrypts the deck with a commutative cipher (`E_A(E_B(m)) = E_B(E_A(m))`,
classically `m^k mod p` for a shared large prime), shuffles, and passes it on; to deal
a card to player *i*, everyone but *i* reveals their key for that card. No ZK proofs,
no threshold crypto — conceptually the simplest scheme, which is exactly why it's the
canonical cautionary tale: the modular-exponentiation instantiation leaks
quadratic-residuosity of each ciphertext, so a cheating player can bias which cards
they end up with. It's textbook, not deployable as described. Any real use needs the
fix (encode each card as multiple residues, or move to a different homomorphism
entirely) — at which point you're most of the way to the next protocol anyway.

### Barnett–Smart, *[Mental Poker Revisited](https://www.semanticscholar.org/paper/Mental-Poker-Revisited-Barnett-Smart/8aaa1245c5876c78564c3f2df36ca615686d1402)*
(2003), and the faster Wei–Wang variant, *[A Fast Mental Poker Protocol](https://eprint.iacr.org/2009/439.pdf)*
(2009, ~2x Barnett–Smart)

The modern baseline. Two primitives: **verifiable threshold masking functions**
(threshold ElGamal — the deck is a list of ElGamal ciphertexts, jointly keyed so no
single player can decrypt alone), and a **zero-knowledge proof of correct shuffle**
(each player, in turn, permutes and re-masks the whole deck and proves they did so
honestly without revealing the permutation). Security reduces to DDH. The shuffle
proof in the original paper is expensive — it's an interactive sigma protocol with
soundness error `2^-L`, so `L` rounds of proof per shuffle, each round redoing a
full re-mask. This is the part later work targeted.

**Bayer–Groth**, *[Efficient Zero-Knowledge Argument for Correctness of a Shuffle](http://www0.cs.ucl.ac.uk/staff/J.Groth/MinimalShuffle.pdf)*
(EUROCRYPT 2012), is the practical fix and the one still used in production
(Verificatum, e-voting systems): a constant-round argument at `O(N log m)`
exponentiations, or `O(N)` with a logarithmic number of rounds; communication can be
pushed down to `O(√N)` at the cost of more computation. `N` here is the deck size —
for a 52-card deck this is a small instance by the standards these papers are built
for (they target `N` in the thousands to millions, for voting).

### SNARK-based re-derivations (Geometry Research, *[Mental Poker in the Age of
SNARKs](https://geometry.xyz/notebook/mental-poker-in-the-age-of-snarks-part-1)*,
2022; zkShuffle, *[Mental Poker on SNARK for Ethereum](https://hackmd.io/@ZDZ-B3ktQlOiBE4iqOXVlg/BJA7Zoqns)*)

Replace the sigma-protocol shuffle proof with a circuit and a general-purpose SNARK
(Groth16, PlonK, or Bulletproofs-style inner-product arguments). Geometry's write-up
reports a naive 52-card shuffle proof at ~50ms generation and <1ms verification on a
laptop — genuinely interactive. zkShuffle's numbers are less favorable for a browser:
their encrypt circuit is 87,308 R1CS constraints and takes 4.5s to prove *in the
DApp*, i.e. in-browser, with the decrypt circuit much cheaper at 1,522 constraints
and 0.1s. Both projects exist because they're building for on-chain settlement
(Ethereum gas costs, not just proving time), which is not this project's shape at
all — we have no chain and no reason to want one.

## Cost for 2–6 players, concretely

Sizing everything to `DECK = 52` (this repo's constant) rather than the
thousands-of-rows scale these papers target:

- **Setup** — each player publishes an ElGamal public-key share; combine into a
  joint key. One round, cheap — structurally like the existing commit phase.
- **Shuffle** — each of the `n` players takes a turn permuting + re-masking the
  *entire* 52-ciphertext deck and attaching a shuffle proof, **sequentially**: player
  2 cannot start until player 1's shuffled-and-proved deck is published. A
  Verificatum-style Bayer–Groth shuffle over Ristretto (same curve family as this
  repo's sr25519 stack) benchmarks at 38.34s prove / 26.43s verify for **100,000
  rows of 6 ciphertexts each** on a laptop CPU (see
  [derbear/verifiable-shuffle](https://github.com/derbear/verifiable-shuffle)); scaled
  linearly down to 52 elements that's comfortably sub-100ms per player. So the
  *computation* is cheap at this deck size — the problem is the `n` sequential
  round-trips it takes to chain the shuffles, where today's engine needs exactly one
  round regardless of `n`.
- **Card opening** — revealing any one hidden card needs a partial-decryption share
  *and a proof it was computed correctly* from every player who isn't the card's
  owner. For hole cards dealt privately that's `n-1` shares per card at deal time;
  at showdown, opening 5 cards for each of `n` players (to compare hands) is
  `O(n)` shares per card × `O(n)` cards ≈ **O(n²)** proof-and-verify operations
  unless shares are batched — which is more engineering, not less.

None of the per-operation costs are prohibitive at `n ≤ 6`. What changes is the
protocol's *shape*: today's `Player::commit → reveal → draw` is one parallel round
for any number of players, and `Transcript` is a handful of fixed-size byte arrays.
Mental poker is inherently `O(n)` sequential rounds for the shuffle chain plus
`O(n²)`-ish card-opening traffic, all of which has to be captured, serialized, and
re-verified.

## Fit with the existing `Transcript`

Not an extension — a different document. `Transcript` today is `pubkeys`,
`commitments`, `reveals`, `preouts`, `proofs`: fixed-width byte arrays, one entry per
player, verified with a handful of hash and VRF checks (`verify_transcript` in
`Poker_VRF/src/lib.rs:566`). A mental-poker transcript would need to carry, at
minimum: the joint public key material, `n` full shuffled-deck snapshots (52
ElGamal ciphertexts each) with `n` shuffle proofs attached, and a decryption-share +
proof for every card ever opened over the course of the hand. That's a different
wire format, a different verifier, and a different set of `Error` variants — the
roadmap's own framing ("almost certainly a new transcript type, not an extension")
holds up under the actual numbers. It would sit alongside `Transcript`, not replace
it, since the infinite-deck demo and Catch the Cheat both depend on the current
one-round shape.

## WASM / interactive-speed verdict

The **sigma-protocol route (ElGamal + Bayer–Groth-style shuffle over Ristretto)** is
the better fit for this codebase specifically: it's the same curve family
`curve25519-dalek`/Ristretto that `schnorrkel` already builds on, needs no new proof
toolchain (no circom, no trusted setup, no R1CS), and the scaled-down numbers above
suggest sub-100ms per shuffle at deck size 52 — genuinely interactive in a browser.
The **SNARK route** would mean adopting a second, unrelated proving stack purely for
this one feature (Geometry's numbers are promising but from a from-scratch circuit;
zkShuffle's in-browser 4.5s-per-encrypt-proof is not interactive), and both SNARK
implementations were built for on-chain gas optimization, a constraint this project
doesn't have and shouldn't import.

Worth naming as a completely different direction: **Bentov et al., *[Instantaneous
Decentralized Poker](https://eprint.iacr.org/2017/875.pdf)*** (2017) sidesteps full
mental-poker shuffling entirely by using a smart contract with financial penalties
plus VRF-extended coin tossing, getting O(1) rounds and O(n) broadcasts. It's
elegant, but it trades cryptographic privacy of the shuffle for an economic-security
model that needs a blockchain and real stakes escrowed on it — the opposite of this
project's "just a browser, no server, no chain" shape. Not a fit here.

## Go / no-go

**No-go**, for three compounding reasons rather than any single blocker:

1. **No ready-made crate.** Unlike VRFs (where `schnorrkel` gave M1 a working,
   if version-sensitive, implementation on day one), there is no maintained,
   audited Rust crate that provides Barnett–Smart-style ElGamal masking +
   Bayer–Groth shuffle proofs + threshold decryption over Ristretto. This would be
   a from-scratch implementation of a protocol family whose own literature is full
   of "the obvious construction leaks information" war stories (SRA's
   quadratic-residuosity leak is the canonical example, discovered *after*
   publication). That is a materially higher correctness bar than anything in
   M1–M3, all of which built on schnorrkel's existing, if finicky, VRF primitives.
2. **Different protocol shape, not just more code.** The engine's whole current
   value proposition — one parallel round of commit → reveal → draw, verifiable by
   anyone in milliseconds — doesn't extend to mental poker. Shuffling is
   inherently sequential in `n`, and card-opening traffic grows with `n²`. That's a
   new network protocol, a new `Transcript` variant, new wasm bindings, and new UI
   for staged per-player turns, on top of the new cryptography.
3. **The project's own verdict is "finish," not "expand the scope."** ROADMAP.md
   already reflects two rounds of deliberately *cutting* scope (betting dropped
   twice) because the infinite-deck hand is strategically thin. Finite-deck mental
   poker is the one change that would actually fix that thinness — but fixing it
   properly is a multi-week, research-grade undertaking for a solo PoC repo, not a
   single PR-sized milestone.

**If this is ever revisited**, the concrete next step is *not* "implement
Barnett–Smart" — it's a much smaller spike: a 2-player-only SRA toy demo that
*also* implements the quadratic-residuosity attack against itself, the same way
Catch the Cheat turns a cryptographic flaw into a playable lesson. That would be
in keeping with what this repo actually does well (making a crypto property
legible by demonstrating its failure mode), costs a fraction of a full
Barnett–Smart implementation, and doesn't pretend to be production mental poker.
It is a *different, smaller* milestone than "add a finite deck," though, and
should be scoped and named as such if Emil wants it — not assumed here.

Absent that, M4's honest close is: the infinite-deck engine, the web demo, and
Catch the Cheat are the shippable result of this project, and this spike is the
record of why it stops there.
