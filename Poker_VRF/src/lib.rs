//! Verifiable infinite-deck poker on sr25519 VRFs (schnorrkel).
//!
//! Protocol (three phases, no player controls their own randomness):
//! 1. **Commit** — every player publishes `c_i = H(domain ‖ pubkey_i ‖ r_i)`
//!    for a secret random `r_i`. Binding to the pubkey stops commitment reuse.
//! 2. **Reveal** — every player publishes `r_i`; everyone checks it against
//!    `c_i`, then derives the shared seed `S = H(domain ‖ all c ‖ all r)`.
//!    Because `S` mixes *all* contributions, no single player can steer it —
//!    the fix for the classic "sign your own commitment" flaw.
//! 3. **Draw** — every player VRF-signs `S`. The VRF output is unpredictable
//!    before reveal yet publicly verifiable after, and it deterministically
//!    maps to a 5-card hand from an *infinite deck* (i.i.d. draws, duplicates
//!    legal — five of a kind is a real hand here). Best hand wins.
//!
//! Everything a game produces is collected in a [`Transcript`] that any third
//! party can re-verify with [`verify_transcript`]. Transcripts serialize to a
//! versioned, hex-encoded JSON document ([`Transcript::to_json`]) so the
//! verification is portable across processes, machines, and languages.

pub mod api;
pub mod cheats;
#[cfg(target_arch = "wasm32")]
mod wasm;

use rand::rngs::OsRng;
use rand::RngCore;
use schnorrkel::{
    context::SigningContext,
    vrf::{VRFInOut, VRFPreOut, VRFProof},
    Keypair, PublicKey,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fmt;

const COMMIT_DOMAIN: &[u8] = b"poker-vrf.commit.v1";
const PASSPHRASE_DOMAIN: &[u8] = b"poker-vrf.passphrase.v1";
const SEED_DOMAIN: &[u8] = b"poker-vrf.seed.v1";
const DRAW_DOMAIN: &[u8] = b"poker-vrf.draw.v1";
/// `VRFInOut::make_bytes` context — the randomness every draw is derived from.
const OUTPUT_DOMAIN: &[u8] = b"poker-vrf.output.v1";
const CARDS_DOMAIN: &[u8] = b"poker-vrf.cards.v1";

const HAND_SIZE: usize = 5;

/// Cards in a suit-complete deck. The deck is infinite, but each *draw* is
/// uniform over these 52 possibilities.
const DECK: u8 = 52;

/// Largest multiple of [`DECK`] that fits in a byte (4 × 52). Bytes at or above
/// this are rejected rather than folded, which is what removes modulo bias —
/// 256 is not a multiple of 52, so folding all 256 values would make the first
/// 48 cards 25% likelier than the last 4.
const REJECTION_BOUND: u8 = 208;

/// Wire-format version written into every serialized [`Transcript`].
pub const TRANSCRIPT_VERSION: u32 = 1;

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

#[derive(Debug, PartialEq, Eq)]
pub enum Error {
    /// A revealed secret does not hash to the published commitment.
    CommitmentMismatch { player: usize },
    /// A VRF proof failed verification against the shared seed.
    BadVrfProof { player: usize },
    /// Byte blobs in a transcript could not be decoded.
    Malformed { what: &'static str },
    /// Transcript arrays disagree in length or are empty.
    BadTranscriptShape,
    /// The transcript's claimed winner does not match recomputation.
    WrongWinner { claimed: usize, actual: usize },
    /// A serialized transcript could not be parsed.
    Encoding { what: String },
    /// A serialized transcript declares a wire version this build cannot read.
    UnsupportedVersion { found: u32, expected: u32 },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::CommitmentMismatch { player } => {
                write!(f, "player {player}: reveal does not match commitment")
            }
            Error::BadVrfProof { player } => {
                write!(f, "player {player}: VRF proof failed verification")
            }
            Error::Malformed { what } => write!(f, "malformed transcript field: {what}"),
            Error::BadTranscriptShape => write!(f, "transcript arrays empty or of unequal length"),
            Error::WrongWinner { claimed, actual } => {
                write!(
                    f,
                    "transcript claims winner {claimed}, recomputation says {actual}"
                )
            }
            Error::Encoding { what } => write!(f, "could not parse transcript: {what}"),
            Error::UnsupportedVersion { found, expected } => write!(
                f,
                "transcript wire version {found} is not supported (this build reads {expected})"
            ),
        }
    }
}

impl std::error::Error for Error {}

// ---------------------------------------------------------------------------
// Cards & hands (infinite deck)
// ---------------------------------------------------------------------------

/// A playing card. `rank` is 2..=14 (14 = ace), `suit` is 0..=3.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Card {
    pub rank: u8,
    pub suit: u8,
}

impl fmt::Display for Card {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let rank = match self.rank {
            2..=10 => return write!(f, "{}{}", self.rank, SUITS[self.suit as usize]),
            11 => "J",
            12 => "Q",
            13 => "K",
            14 => "A",
            _ => "?",
        };
        write!(f, "{}{}", rank, SUITS[self.suit as usize])
    }
}

const SUITS: [&str; 4] = ["♣", "♦", "♥", "♠"];

/// Hand categories, weakest to strongest. Because the deck is infinite,
/// duplicates are legal and five of a kind outranks a straight flush.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum HandCategory {
    HighCard,
    OnePair,
    TwoPair,
    ThreeOfAKind,
    Straight,
    Flush,
    FullHouse,
    FourOfAKind,
    StraightFlush,
    FiveOfAKind,
}

/// A fully ordered hand evaluation: category first, then kicker ranks.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct HandRank {
    pub category: HandCategory,
    /// Ranks grouped by (count desc, rank desc), flattened — a total tiebreak
    /// order within a category (wheel straights encode the ace as 1).
    pub tiebreak: Vec<u8>,
}

/// Evaluate a 5-card hand (duplicates allowed).
pub fn evaluate_hand(cards: &[Card; HAND_SIZE]) -> HandRank {
    let mut counts: Vec<(u8, u8)> = Vec::new(); // (rank, count)
    for c in cards {
        match counts.iter_mut().find(|(r, _)| *r == c.rank) {
            Some((_, n)) => *n += 1,
            None => counts.push((c.rank, 1)),
        }
    }
    // (count desc, rank desc)
    counts.sort_by(|a, b| b.1.cmp(&a.1).then(b.0.cmp(&a.0)));

    let flush = cards.iter().all(|c| c.suit == cards[0].suit);
    let mut ranks: Vec<u8> = cards.iter().map(|c| c.rank).collect();
    ranks.sort_unstable();
    ranks.dedup();
    let straight_high = if ranks.len() == HAND_SIZE {
        if ranks[4] - ranks[0] == 4 {
            Some(ranks[4])
        } else if ranks == [2, 3, 4, 5, 14] {
            Some(5) // wheel: A-2-3-4-5, the five plays high
        } else {
            None
        }
    } else {
        None
    };

    let tiebreak: Vec<u8> = counts.iter().map(|(r, _)| *r).collect();
    let category = match (
        counts[0].1,
        counts.get(1).map(|c| c.1),
        flush,
        straight_high,
    ) {
        (5, ..) => HandCategory::FiveOfAKind,
        (_, _, true, Some(_)) => HandCategory::StraightFlush,
        (4, ..) => HandCategory::FourOfAKind,
        (3, Some(2), ..) => HandCategory::FullHouse,
        (_, _, true, None) => HandCategory::Flush,
        (_, _, false, Some(_)) => HandCategory::Straight,
        (3, ..) => HandCategory::ThreeOfAKind,
        (2, Some(2), ..) => HandCategory::TwoPair,
        (2, ..) => HandCategory::OnePair,
        _ => HandCategory::HighCard,
    };

    let tiebreak = match straight_high {
        Some(high)
            if category == HandCategory::Straight || category == HandCategory::StraightFlush =>
        {
            vec![high]
        }
        _ => tiebreak,
    };

    HandRank { category, tiebreak }
}

/// Deterministically map a verified VRF output to a 5-card hand.
///
/// The input is the output of [`VRFInOut::make_bytes`] (see [`vrf_output`]),
/// *not* the raw pre-output: `make_bytes` is the 2Hash-DH construction, which
/// commits to the VRF input as well as the output, whereas a bare `VRFPreOut`
/// is only a compressed group element that does not bind the input.
///
/// Entropy is a SHA-256 chain over that output; each byte below 208 (= 4·52)
/// yields one unbiased card via modulo, others are rejected (no modulo bias).
pub fn cards_from_vrf_output(vrf_output: &[u8; 32]) -> [Card; HAND_SIZE] {
    let mut cards = [Card { rank: 2, suit: 0 }; HAND_SIZE];
    let mut found = 0;
    let mut block: [u8; 32] = Sha256::new()
        .chain_update(CARDS_DOMAIN)
        .chain_update(vrf_output)
        .finalize()
        .into();
    loop {
        for byte in block {
            if byte < REJECTION_BOUND {
                let idx = byte % DECK;
                cards[found] = Card {
                    rank: 2 + idx % 13,
                    suit: idx / 13,
                };
                found += 1;
                if found == HAND_SIZE {
                    return cards;
                }
            }
        }
        block = Sha256::digest(block).into();
    }
}

// ---------------------------------------------------------------------------
// Protocol
// ---------------------------------------------------------------------------

/// A player's keypair plus their secret commitment contribution.
pub struct Player {
    keypair: Keypair,
    secret: Option<[u8; 32]>,
    preset: Option<[u8; 32]>,
}

impl Player {
    pub fn new() -> Self {
        Self {
            keypair: Keypair::generate_with(OsRng),
            secret: None,
            preset: None,
        }
    }

    /// Deterministic player for tests.
    pub fn from_seed(seed: [u8; 32]) -> Self {
        let mini = schnorrkel::MiniSecretKey::from_bytes(&seed).expect("32 bytes");
        Self {
            keypair: mini.expand_to_keypair(schnorrkel::ExpansionMode::Uniform),
            secret: None,
            preset: None,
        }
    }

    /// Supply this player's own secret contribution for the next commit,
    /// instead of letting [`Player::commit`] draw one from the system RNG.
    ///
    /// This is the one step a human can perform that materially affects the
    /// protocol: it is what lets you personally know the seed was not steered,
    /// rather than taking the process's word for it.
    ///
    /// The value is consumed by the next [`Player::commit`] — a later round
    /// draws fresh randomness, because reusing a commit-reveal secret across
    /// rounds would let anyone who saw the first reveal predict the second.
    pub fn preset_secret(&mut self, secret: [u8; 32]) {
        self.preset = Some(secret);
    }

    pub fn public(&self) -> [u8; 32] {
        self.keypair.public.to_bytes()
    }

    /// Phase 1: publish a commitment to a secret contribution, domain-separated
    /// and bound to this player's public key.
    ///
    /// Uses the secret from [`Player::preset_secret`] if one is waiting,
    /// otherwise draws a fresh one from the system RNG.
    pub fn commit(&mut self) -> [u8; 32] {
        let secret = self.preset.take().unwrap_or_else(|| {
            let mut fresh = [0u8; 32];
            OsRng.fill_bytes(&mut fresh);
            fresh
        });
        self.secret = Some(secret);
        commitment_hash(&self.public(), &secret)
    }

    /// Phase 2: reveal the secret contribution.
    pub fn reveal(&self) -> [u8; 32] {
        self.secret.expect("reveal called before commit")
    }

    /// Phase 3: VRF-sign the shared seed.
    ///
    /// Returns the pre-output and proof (both go in the transcript) plus the
    /// derived VRF output the hand is dealt from. A verifier recomputes that
    /// same output from the pre-output and proof — see [`verify_draw`].
    pub fn draw(&self, seed: &[u8; 32]) -> Draw {
        let ctx = SigningContext::new(DRAW_DOMAIN);
        let (inout, proof, _) = self.keypair.vrf_sign(ctx.bytes(seed));
        Draw {
            preout: inout.to_preout().to_bytes(),
            proof: proof.to_bytes(),
            output: vrf_output(&inout),
        }
    }
}

/// One player's VRF draw: what goes on the wire, plus the randomness it yields.
#[derive(Debug, Clone, Copy)]
pub struct Draw {
    pub preout: [u8; 32],
    pub proof: [u8; 64],
    pub output: [u8; 32],
}

/// The canonical randomness of a VRF draw: `make_bytes` under [`OUTPUT_DOMAIN`].
fn vrf_output(inout: &VRFInOut) -> [u8; 32] {
    inout.make_bytes(OUTPUT_DOMAIN)
}

impl Default for Player {
    fn default() -> Self {
        Self::new()
    }
}

/// Derive a 32-byte secret contribution from a human-typed passphrase, so a
/// person can supply their own entropy to [`Player::preset_secret`].
///
/// This is a plain domain-separated hash, *not* a password KDF — a guessable
/// passphrase yields a guessable contribution. That is survivable here (the
/// seed stays unpredictable as long as *any* participant contributed real
/// randomness) but it means your own contribution is only as unpredictable as
/// what you typed. Prefer the system RNG unless you specifically want to be
/// able to reproduce a game.
pub fn secret_from_passphrase(passphrase: &str) -> [u8; 32] {
    Sha256::new()
        .chain_update(PASSPHRASE_DOMAIN)
        .chain_update(passphrase.as_bytes())
        .finalize()
        .into()
}

fn commitment_hash(pubkey: &[u8; 32], secret: &[u8; 32]) -> [u8; 32] {
    Sha256::new()
        .chain_update(COMMIT_DOMAIN)
        .chain_update(pubkey)
        .chain_update(secret)
        .finalize()
        .into()
}

/// Derive the shared seed from every commitment and every reveal.
pub fn combine_seed(commitments: &[[u8; 32]], reveals: &[[u8; 32]]) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(SEED_DOMAIN);
    for c in commitments {
        h.update(c);
    }
    for r in reveals {
        h.update(r);
    }
    h.finalize().into()
}

/// Verify one player's VRF draw against the shared seed.
///
/// On success returns the draw's VRF output — the randomness the hand is dealt
/// from. Deriving cards from this (rather than from the pre-output bytes the
/// transcript carries) means a hand can only be computed *after* the proof has
/// been checked.
pub fn verify_draw(
    player: usize,
    pubkey: &[u8; 32],
    seed: &[u8; 32],
    preout: &[u8; 32],
    proof: &[u8; 64],
) -> Result<[u8; 32], Error> {
    let pk = PublicKey::from_bytes(pubkey).map_err(|_| Error::Malformed { what: "pubkey" })?;
    let po = VRFPreOut::from_bytes(preout).map_err(|_| Error::Malformed { what: "preout" })?;
    let pr = VRFProof::from_bytes(proof).map_err(|_| Error::Malformed { what: "proof" })?;
    let ctx = SigningContext::new(DRAW_DOMAIN);
    let (inout, _) = pk
        .vrf_verify(ctx.bytes(seed), &po, &pr)
        .map_err(|_| Error::BadVrfProof { player })?;
    Ok(vrf_output(&inout))
}

// ---------------------------------------------------------------------------
// Game + transcript
// ---------------------------------------------------------------------------

/// Everything needed for a third party to re-verify a finished game.
///
/// Serializes to versioned JSON with every byte string hex-encoded — see
/// [`Transcript::to_json`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Transcript {
    #[serde(with = "hex_arrays")]
    pub pubkeys: Vec<[u8; 32]>,
    #[serde(with = "hex_arrays")]
    pub commitments: Vec<[u8; 32]>,
    #[serde(with = "hex_arrays")]
    pub reveals: Vec<[u8; 32]>,
    #[serde(with = "hex_arrays")]
    pub preouts: Vec<[u8; 32]>,
    #[serde(with = "hex_arrays")]
    pub proofs: Vec<[u8; 64]>,
    pub winner: usize,
}

/// A transcript as it appears on the wire, tagged with the format version.
#[derive(Serialize, Deserialize)]
struct TranscriptDoc {
    version: u32,
    #[serde(flatten)]
    transcript: Transcript,
}

impl Transcript {
    /// Encode as pretty-printed JSON: a `version` tag plus hex byte strings.
    ///
    /// The encoding does not need to be byte-canonical — [`verify_transcript`]
    /// re-derives everything from the decoded fields, never from the document
    /// text, so reformatting a transcript cannot change whether it verifies.
    pub fn to_json(&self) -> String {
        let doc = TranscriptDoc {
            version: TRANSCRIPT_VERSION,
            transcript: self.clone(),
        };
        serde_json::to_string_pretty(&doc).expect("Transcript is always serializable")
    }

    /// Decode a transcript produced by [`Transcript::to_json`].
    ///
    /// Decoding only checks that the document is well-formed and that every
    /// field has the right length — it says nothing about whether the game was
    /// honest. Pass the result to [`verify_transcript`] for that.
    pub fn from_json(s: &str) -> Result<Self, Error> {
        let doc: TranscriptDoc = serde_json::from_str(s).map_err(|e| Error::Encoding {
            what: e.to_string(),
        })?;
        if doc.version != TRANSCRIPT_VERSION {
            return Err(Error::UnsupportedVersion {
                found: doc.version,
                expected: TRANSCRIPT_VERSION,
            });
        }
        Ok(doc.transcript)
    }
}

/// Serialize `Vec<[u8; N]>` as an array of hex strings.
mod hex_arrays {
    use serde::{de::Error as _, Deserialize, Deserializer, Serializer};

    pub fn serialize<S, const N: usize>(v: &[[u8; N]], s: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        s.collect_seq(v.iter().map(hex::encode))
    }

    pub fn deserialize<'de, D, const N: usize>(d: D) -> Result<Vec<[u8; N]>, D::Error>
    where
        D: Deserializer<'de>,
    {
        Vec::<String>::deserialize(d)?
            .into_iter()
            .map(|s| {
                let bytes = hex::decode(&s).map_err(D::Error::custom)?;
                <[u8; N]>::try_from(bytes.as_slice()).map_err(|_| {
                    D::Error::custom(format!("expected {N} bytes, got {}", bytes.len()))
                })
            })
            .collect()
    }
}

/// The verified outcome of a game.
#[derive(Debug)]
pub struct Outcome {
    pub seed: [u8; 32],
    pub hands: Vec<[Card; HAND_SIZE]>,
    pub ranks: Vec<HandRank>,
    pub winner: usize,
}

/// Decide the winner from verified VRF outputs: best hand wins; exact hand ties
/// fall back to lexicographic comparison of the outputs (deterministic).
fn decide_winner(outputs: &[[u8; 32]], ranks: &[HandRank]) -> usize {
    let mut best = 0;
    for i in 1..ranks.len() {
        if (&ranks[i], &outputs[i]) > (&ranks[best], &outputs[best]) {
            best = i;
        }
    }
    best
}

/// Play a full game among `n` players and produce the transcript + outcome.
pub fn play_game(players: &mut [Player]) -> (Transcript, Outcome) {
    let commitments: Vec<[u8; 32]> = players.iter_mut().map(|p| p.commit()).collect();
    let reveals: Vec<[u8; 32]> = players.iter().map(|p| p.reveal()).collect();
    let seed = combine_seed(&commitments, &reveals);

    let draws: Vec<Draw> = players.iter().map(|p| p.draw(&seed)).collect();
    let preouts: Vec<[u8; 32]> = draws.iter().map(|d| d.preout).collect();
    let proofs: Vec<[u8; 64]> = draws.iter().map(|d| d.proof).collect();
    let outputs: Vec<[u8; 32]> = draws.iter().map(|d| d.output).collect();

    let hands: Vec<[Card; HAND_SIZE]> = outputs.iter().map(cards_from_vrf_output).collect();
    let ranks: Vec<HandRank> = hands.iter().map(evaluate_hand).collect();
    let winner = decide_winner(&outputs, &ranks);

    let transcript = Transcript {
        pubkeys: players.iter().map(|p| p.public()).collect(),
        commitments,
        reveals,
        preouts,
        proofs,
        winner,
    };
    let outcome = Outcome {
        seed,
        hands,
        ranks,
        winner,
    };
    (transcript, outcome)
}

/// Re-verify a transcript from scratch: commitments, seed, every VRF proof,
/// every hand, and the winner. Returns the recomputed outcome on success.
pub fn verify_transcript(t: &Transcript) -> Result<Outcome, Error> {
    let n = t.pubkeys.len();
    if n == 0
        || t.commitments.len() != n
        || t.reveals.len() != n
        || t.preouts.len() != n
        || t.proofs.len() != n
        || t.winner >= n
    {
        return Err(Error::BadTranscriptShape);
    }

    for i in 0..n {
        if commitment_hash(&t.pubkeys[i], &t.reveals[i]) != t.commitments[i] {
            return Err(Error::CommitmentMismatch { player: i });
        }
    }

    let seed = combine_seed(&t.commitments, &t.reveals);

    let mut outputs = Vec::with_capacity(n);
    for i in 0..n {
        outputs.push(verify_draw(
            i,
            &t.pubkeys[i],
            &seed,
            &t.preouts[i],
            &t.proofs[i],
        )?);
    }

    let hands: Vec<[Card; HAND_SIZE]> = outputs.iter().map(cards_from_vrf_output).collect();
    let ranks: Vec<HandRank> = hands.iter().map(evaluate_hand).collect();
    let winner = decide_winner(&outputs, &ranks);
    if winner != t.winner {
        return Err(Error::WrongWinner {
            claimed: t.winner,
            actual: winner,
        });
    }

    Ok(Outcome {
        seed,
        hands,
        ranks,
        winner,
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn c(rank: u8, suit: u8) -> Card {
        Card { rank, suit }
    }

    #[test]
    fn hand_categories_rank_correctly() {
        let five = evaluate_hand(&[c(9, 0), c(9, 1), c(9, 2), c(9, 3), c(9, 0)]);
        let sflush = evaluate_hand(&[c(5, 2), c(6, 2), c(7, 2), c(8, 2), c(9, 2)]);
        let quads = evaluate_hand(&[c(4, 0), c(4, 1), c(4, 2), c(4, 3), c(11, 0)]);
        let full = evaluate_hand(&[c(3, 0), c(3, 1), c(3, 2), c(8, 0), c(8, 1)]);
        let flush = evaluate_hand(&[c(2, 3), c(6, 3), c(9, 3), c(11, 3), c(13, 3)]);
        let straight = evaluate_hand(&[c(5, 0), c(6, 1), c(7, 2), c(8, 3), c(9, 0)]);
        let wheel = evaluate_hand(&[c(14, 0), c(2, 1), c(3, 2), c(4, 3), c(5, 0)]);
        let trips = evaluate_hand(&[c(7, 0), c(7, 1), c(7, 2), c(2, 0), c(9, 1)]);
        let two_pair = evaluate_hand(&[c(10, 0), c(10, 1), c(4, 2), c(4, 0), c(9, 1)]);
        let pair = evaluate_hand(&[c(12, 0), c(12, 1), c(4, 2), c(7, 0), c(9, 1)]);
        let high = evaluate_hand(&[c(2, 0), c(6, 1), c(9, 2), c(11, 0), c(13, 1)]);

        let mut order = vec![
            &high, &pair, &two_pair, &trips, &straight, &flush, &full, &quads, &sflush, &five,
        ];
        let sorted = order.clone();
        order.sort();
        assert_eq!(
            order, sorted,
            "categories must already be in ascending order"
        );
        assert!(
            wheel > high && wheel < straight,
            "wheel is the lowest straight"
        );
    }

    #[test]
    fn straight_tiebreak_uses_high_card() {
        let nine_high = evaluate_hand(&[c(5, 0), c(6, 1), c(7, 2), c(8, 3), c(9, 0)]);
        let ten_high = evaluate_hand(&[c(6, 0), c(7, 1), c(8, 2), c(9, 3), c(10, 0)]);
        assert!(ten_high > nine_high);
    }

    #[test]
    fn card_mapping_is_deterministic_and_in_range() {
        let output = [42u8; 32];
        let a = cards_from_vrf_output(&output);
        let b = cards_from_vrf_output(&output);
        assert_eq!(a, b);
        for card in a {
            assert!((2..=14).contains(&card.rank));
            assert!(card.suit < 4);
        }
        assert_ne!(
            a,
            cards_from_vrf_output(&[43u8; 32]),
            "different VRF output, different hand"
        );
    }

    /// Index a card the way `cards_from_vrf_output` does: `idx = suit*13 + rank-2`.
    fn deck_index(c: &Card) -> usize {
        c.suit as usize * 13 + (c.rank - 2) as usize
    }

    /// Pearson's chi-square against a uniform distribution over 52 outcomes.
    fn chi_square_vs_uniform(counts: &[u32; 52]) -> f64 {
        let total: u32 = counts.iter().sum();
        let expected = f64::from(total) / 52.0;
        counts
            .iter()
            .map(|&observed| {
                let d = f64::from(observed) - expected;
                d * d / expected
            })
            .sum()
    }

    /// 51 degrees of freedom. The chi-square critical value at p = 0.001 is
    /// ~86.7; 110 leaves generous headroom while still being far below what
    /// real modulo bias produces — see `the_uniformity_test_can_detect_bias`.
    const CHI2_THRESHOLD: f64 = 110.0;

    #[test]
    fn card_sampling_is_uniform_over_the_deck() {
        // Deterministic inputs, so this test cannot flake: it either passes for
        // everyone forever or it is a real regression.
        let mut counts = [0u32; 52];
        for i in 0..20_000u64 {
            let output: [u8; 32] = Sha256::digest(i.to_le_bytes()).into();
            for card in cards_from_vrf_output(&output) {
                counts[deck_index(&card)] += 1;
            }
        }

        assert!(
            counts.iter().all(|&n| n > 0),
            "every card in the deck must be reachable"
        );

        let chi2 = chi_square_vs_uniform(&counts);
        assert!(
            chi2 < CHI2_THRESHOLD,
            "card distribution is not uniform: chi-square {chi2:.1} over 51 df \
             exceeds {CHI2_THRESHOLD}. The README claims rejection sampling \
             removes modulo bias; this says otherwise."
        );
    }

    #[test]
    fn the_uniformity_test_can_detect_bias() {
        // A test that only ever passes proves nothing. This runs the *naive*
        // mapping the real code deliberately avoids — `byte % 52` with no
        // rejection — and requires the same threshold to reject it.
        //
        // 256 = 4*52 + 48, so indices 0..47 land five times per 256 bytes and
        // 48..51 only four: a 5:4 bias, exactly what rejection sampling exists
        // to remove.
        let mut counts = [0u32; 52];
        for i in 0..20_000u64 {
            let mut block: [u8; 32] = Sha256::digest(i.to_le_bytes()).into();
            let mut taken = 0;
            while taken < HAND_SIZE {
                for byte in block {
                    counts[(byte % 52) as usize] += 1;
                    taken += 1;
                    if taken == HAND_SIZE {
                        break;
                    }
                }
                block = Sha256::digest(block).into();
            }
        }

        let chi2 = chi_square_vs_uniform(&counts);
        assert!(
            chi2 > CHI2_THRESHOLD,
            "the biased sampler scored chi-square {chi2:.1}, below the {CHI2_THRESHOLD} \
             threshold — so `card_sampling_is_uniform_over_the_deck` could not have \
             detected real modulo bias either, and proves nothing"
        );
    }

    #[test]
    fn the_rejection_bound_is_the_largest_usable_multiple() {
        // Derived, not restated: recompute the bound from the deck size rather
        // than asserting facts about literals. Changing REJECTION_BOUND to
        // anything that is not the largest multiple of DECK in a byte fails here.
        let largest_multiple = (256u16 / u16::from(DECK)) * u16::from(DECK);
        assert_eq!(
            u16::from(REJECTION_BOUND),
            largest_multiple,
            "the bound must be the largest multiple of {DECK} that fits in a byte"
        );

        // And the property that buys: every accepted byte maps to a card, and
        // each card is reachable from exactly the same number of bytes.
        let mut hits = [0u32; DECK as usize];
        for byte in 0..=u8::MAX {
            if byte < REJECTION_BOUND {
                hits[(byte % DECK) as usize] += 1;
            }
        }
        let first = hits[0];
        assert!(
            hits.iter().all(|&h| h == first),
            "accepted bytes must cover every card equally: {hits:?}"
        );
    }

    #[test]
    fn signer_and_verifier_derive_the_same_vrf_output() {
        let player = Player::from_seed([3; 32]);
        let seed = [77u8; 32];
        let draw = player.draw(&seed);
        let verified = verify_draw(0, &player.public(), &seed, &draw.preout, &draw.proof)
            .expect("honest draw verifies");
        assert_eq!(
            draw.output, verified,
            "hand must not depend on who computed it"
        );
    }

    #[test]
    fn vrf_output_is_not_the_raw_preout() {
        // make_bytes commits to input *and* output; the pre-output is only the
        // output point. Confusing the two is the bug this guards against.
        let player = Player::from_seed([5; 32]);
        let draw = player.draw(&[1u8; 32]);
        assert_ne!(draw.output, draw.preout);
    }

    #[test]
    fn same_key_different_seed_gives_different_hand() {
        let player = Player::from_seed([11; 32]);
        let a = player.draw(&[1u8; 32]);
        let b = player.draw(&[2u8; 32]);
        assert_ne!(
            cards_from_vrf_output(&a.output),
            cards_from_vrf_output(&b.output)
        );
    }

    #[test]
    fn full_game_roundtrip_verifies() {
        let mut players: Vec<Player> = (0..4u8).map(|i| Player::from_seed([i; 32])).collect();
        let (transcript, outcome) = play_game(&mut players);
        let reverified = verify_transcript(&transcript).expect("honest transcript verifies");
        assert_eq!(reverified.winner, outcome.winner);
        assert_eq!(reverified.hands, outcome.hands);
        assert_eq!(reverified.seed, outcome.seed);
    }

    #[test]
    fn seed_mixes_every_contribution() {
        let mut players: Vec<Player> = (0..3u8).map(|i| Player::from_seed([i; 32])).collect();
        let commitments: Vec<_> = players.iter_mut().map(|p| p.commit()).collect();
        let reveals: Vec<_> = players.iter().map(|p| p.reveal()).collect();
        let seed = combine_seed(&commitments, &reveals);

        let mut tampered = reveals.clone();
        tampered[2][0] ^= 1;
        assert_ne!(seed, combine_seed(&commitments, &tampered));
    }

    #[test]
    fn tampered_reveal_is_rejected() {
        let mut players: Vec<Player> = (0..3u8).map(|i| Player::from_seed([i; 32])).collect();
        let (mut t, _) = play_game(&mut players);
        t.reveals[1][0] ^= 0xff;
        assert!(matches!(
            verify_transcript(&t),
            Err(Error::CommitmentMismatch { player: 1 })
        ));
    }

    #[test]
    fn forged_vrf_output_is_rejected() {
        let mut players: Vec<Player> = (0..3u8).map(|i| Player::from_seed([i; 32])).collect();
        let (mut t, _) = play_game(&mut players);
        // Player 0 tries to swap in a "better" pre-output without a valid proof.
        t.preouts[0] = [0xee; 32];
        match verify_transcript(&t) {
            Err(Error::BadVrfProof { player: 0 }) | Err(Error::Malformed { .. }) => {}
            other => panic!("forgery must fail verification, got {other:?}"),
        }
    }

    #[test]
    fn misdeclared_winner_is_rejected() {
        let mut players: Vec<Player> = (0..3u8).map(|i| Player::from_seed([i; 32])).collect();
        let (mut t, outcome) = play_game(&mut players);
        t.winner = (outcome.winner + 1) % 3;
        assert!(matches!(
            verify_transcript(&t),
            Err(Error::WrongWinner { .. })
        ));
    }

    #[test]
    fn preset_secret_is_used_and_then_consumed() {
        let mut p = Player::from_seed([21; 32]);
        let mine = secret_from_passphrase("correct horse battery staple");
        p.preset_secret(mine);

        let commitment = p.commit();
        assert_eq!(p.reveal(), mine, "commit must use the secret I supplied");
        assert_eq!(commitment, commitment_hash(&p.public(), &mine));

        // Round two: the preset is spent, so fresh randomness is drawn.
        // Reusing a commit-reveal secret would make the next round predictable
        // to anyone who saw the first reveal.
        p.commit();
        assert_ne!(p.reveal(), mine, "preset must not carry into a later round");
    }

    #[test]
    fn a_human_contribution_changes_the_whole_deal() {
        let deal = |passphrase: &str| {
            let mut players: Vec<Player> = (0..3u8).map(|i| Player::from_seed([i; 32])).collect();
            players[0].preset_secret(secret_from_passphrase(passphrase));
            let (t, outcome) = play_game(&mut players);
            verify_transcript(&t).expect("verifies");
            outcome.hands
        };
        assert_ne!(
            deal("hunter2"),
            deal("hunter3"),
            "one character of my passphrase must change the deal"
        );
    }

    #[test]
    fn passphrase_derivation_is_deterministic_and_separated() {
        assert_eq!(
            secret_from_passphrase("same"),
            secret_from_passphrase("same")
        );
        assert_ne!(secret_from_passphrase("a"), secret_from_passphrase("b"));
        // Domain separation: not a bare SHA-256 of the passphrase.
        let bare: [u8; 32] = Sha256::digest(b"a").into();
        assert_ne!(secret_from_passphrase("a"), bare);
    }

    #[test]
    fn transcript_survives_a_json_roundtrip() {
        let mut players: Vec<Player> = (0..4u8).map(|i| Player::from_seed([i; 32])).collect();
        let (t, outcome) = play_game(&mut players);

        let decoded = Transcript::from_json(&t.to_json()).expect("roundtrip decodes");
        assert_eq!(decoded, t);

        // The real point: a transcript that crossed the wire still verifies.
        let reverified = verify_transcript(&decoded).expect("decoded transcript verifies");
        assert_eq!(reverified.winner, outcome.winner);
        assert_eq!(reverified.hands, outcome.hands);
        assert_eq!(reverified.seed, outcome.seed);
    }

    #[test]
    fn reformatted_json_still_verifies() {
        let mut players: Vec<Player> = (0..3u8).map(|i| Player::from_seed([i; 32])).collect();
        let (t, _) = play_game(&mut players);

        // Whitespace is not part of the security surface — verification reads
        // decoded fields, never the document text.
        let compact = t.to_json().replace(['\n', ' '], "");
        let decoded = Transcript::from_json(&compact).expect("compact JSON decodes");
        assert!(verify_transcript(&decoded).is_ok());
    }

    #[test]
    fn tampering_with_serialized_json_is_caught() {
        let mut players: Vec<Player> = (0..3u8).map(|i| Player::from_seed([i; 32])).collect();
        let (t, _) = play_game(&mut players);

        // Flip one hex nibble of player 1's reveal inside the document.
        let original = hex::encode(t.reveals[1]);
        let mut flipped: Vec<char> = original.chars().collect();
        flipped[0] = if flipped[0] == '0' { '1' } else { '0' };
        let flipped: String = flipped.into_iter().collect();
        let json = t.to_json().replace(&original, &flipped);

        let decoded = Transcript::from_json(&json).expect("still well-formed JSON");
        assert!(matches!(
            verify_transcript(&decoded),
            Err(Error::CommitmentMismatch { player: 1 })
        ));
    }

    #[test]
    fn malformed_documents_are_rejected() {
        let mut players: Vec<Player> = (0..3u8).map(|i| Player::from_seed([i; 32])).collect();
        let (t, _) = play_game(&mut players);
        let json = t.to_json();

        assert!(matches!(
            Transcript::from_json("not json at all"),
            Err(Error::Encoding { .. })
        ));
        // Truncated field: right hex, wrong length.
        let short = json.replace(&hex::encode(t.pubkeys[0]), "abcd");
        assert!(matches!(
            Transcript::from_json(&short),
            Err(Error::Encoding { .. })
        ));
        // Non-hex characters.
        let non_hex = json.replace(&hex::encode(t.proofs[0]), "zz");
        assert!(matches!(
            Transcript::from_json(&non_hex),
            Err(Error::Encoding { .. })
        ));
    }

    #[test]
    fn future_wire_versions_are_refused() {
        let mut players: Vec<Player> = (0..3u8).map(|i| Player::from_seed([i; 32])).collect();
        let (t, _) = play_game(&mut players);
        let bumped = t.to_json().replace(
            &format!("\"version\": {TRANSCRIPT_VERSION}"),
            "\"version\": 99",
        );
        assert!(matches!(
            Transcript::from_json(&bumped),
            Err(Error::UnsupportedVersion {
                found: 99,
                expected: TRANSCRIPT_VERSION
            })
        ));
    }

    #[test]
    fn commitment_is_bound_to_pubkey() {
        let mut a = Player::from_seed([7; 32]);
        let commitment = a.commit();
        let secret = a.reveal();
        let b_pub = Player::from_seed([9; 32]).public();
        assert_ne!(
            commitment_hash(&b_pub, &secret),
            commitment,
            "another player cannot claim the same commitment"
        );
    }
}
