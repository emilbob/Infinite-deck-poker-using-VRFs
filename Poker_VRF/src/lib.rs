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
//! party can re-verify with [`verify_transcript`].

use rand::rngs::OsRng;
use rand::RngCore;
use schnorrkel::{
    context::SigningContext,
    vrf::{VRFPreOut, VRFProof},
    Keypair, PublicKey,
};
use sha2::{Digest, Sha256};
use std::fmt;

const COMMIT_DOMAIN: &[u8] = b"poker-vrf.commit.v1";
const SEED_DOMAIN: &[u8] = b"poker-vrf.seed.v1";
const DRAW_DOMAIN: &[u8] = b"poker-vrf.draw.v1";
const CARDS_DOMAIN: &[u8] = b"poker-vrf.cards.v1";

const HAND_SIZE: usize = 5;

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
        }
    }
}

impl std::error::Error for Error {}

// ---------------------------------------------------------------------------
// Cards & hands (infinite deck)
// ---------------------------------------------------------------------------

/// A playing card. `rank` is 2..=14 (14 = ace), `suit` is 0..=3.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

/// Deterministically map a verified VRF pre-output to a 5-card hand.
///
/// Entropy is a SHA-256 chain over the pre-output; each byte below 208 (= 4·52)
/// yields one unbiased card via modulo, others are rejected (no modulo bias).
pub fn cards_from_preout(preout: &[u8; 32]) -> [Card; HAND_SIZE] {
    let mut cards = [Card { rank: 2, suit: 0 }; HAND_SIZE];
    let mut found = 0;
    let mut block: [u8; 32] = Sha256::new()
        .chain_update(CARDS_DOMAIN)
        .chain_update(preout)
        .finalize()
        .into();
    loop {
        for byte in block {
            if byte < 208 {
                let idx = byte % 52;
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
}

impl Player {
    pub fn new() -> Self {
        Self {
            keypair: Keypair::generate_with(OsRng),
            secret: None,
        }
    }

    /// Deterministic player for tests.
    pub fn from_seed(seed: [u8; 32]) -> Self {
        let mini = schnorrkel::MiniSecretKey::from_bytes(&seed).expect("32 bytes");
        Self {
            keypair: mini.expand_to_keypair(schnorrkel::ExpansionMode::Uniform),
            secret: None,
        }
    }

    pub fn public(&self) -> [u8; 32] {
        self.keypair.public.to_bytes()
    }

    /// Phase 1: draw a secret contribution and publish its commitment,
    /// domain-separated and bound to this player's public key.
    pub fn commit(&mut self) -> [u8; 32] {
        let mut secret = [0u8; 32];
        OsRng.fill_bytes(&mut secret);
        self.secret = Some(secret);
        commitment_hash(&self.public(), &secret)
    }

    /// Phase 2: reveal the secret contribution.
    pub fn reveal(&self) -> [u8; 32] {
        self.secret.expect("reveal called before commit")
    }

    /// Phase 3: VRF-sign the shared seed, producing (pre-output, proof).
    pub fn draw(&self, seed: &[u8; 32]) -> ([u8; 32], [u8; 64]) {
        let ctx = SigningContext::new(DRAW_DOMAIN);
        let (inout, proof, _) = self.keypair.vrf_sign(ctx.bytes(seed));
        (inout.to_preout().to_bytes(), proof.to_bytes())
    }
}

impl Default for Player {
    fn default() -> Self {
        Self::new()
    }
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
pub fn verify_draw(
    player: usize,
    pubkey: &[u8; 32],
    seed: &[u8; 32],
    preout: &[u8; 32],
    proof: &[u8; 64],
) -> Result<(), Error> {
    let pk = PublicKey::from_bytes(pubkey).map_err(|_| Error::Malformed { what: "pubkey" })?;
    let po = VRFPreOut::from_bytes(preout).map_err(|_| Error::Malformed { what: "preout" })?;
    let pr = VRFProof::from_bytes(proof).map_err(|_| Error::Malformed { what: "proof" })?;
    let ctx = SigningContext::new(DRAW_DOMAIN);
    pk.vrf_verify(ctx.bytes(seed), &po, &pr)
        .map(|_| ())
        .map_err(|_| Error::BadVrfProof { player })
}

// ---------------------------------------------------------------------------
// Game + transcript
// ---------------------------------------------------------------------------

/// Everything needed for a third party to re-verify a finished game.
#[derive(Debug, Clone)]
pub struct Transcript {
    pub pubkeys: Vec<[u8; 32]>,
    pub commitments: Vec<[u8; 32]>,
    pub reveals: Vec<[u8; 32]>,
    pub preouts: Vec<[u8; 32]>,
    pub proofs: Vec<[u8; 64]>,
    pub winner: usize,
}

/// The verified outcome of a game.
#[derive(Debug)]
pub struct Outcome {
    pub seed: [u8; 32],
    pub hands: Vec<[Card; HAND_SIZE]>,
    pub ranks: Vec<HandRank>,
    pub winner: usize,
}

/// Decide the winner from verified pre-outputs: best hand wins; exact hand
/// ties fall back to lexicographic pre-output comparison (deterministic).
fn decide_winner(preouts: &[[u8; 32]], ranks: &[HandRank]) -> usize {
    let mut best = 0;
    for i in 1..ranks.len() {
        if (&ranks[i], &preouts[i]) > (&ranks[best], &preouts[best]) {
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

    let draws: Vec<([u8; 32], [u8; 64])> = players.iter().map(|p| p.draw(&seed)).collect();
    let preouts: Vec<[u8; 32]> = draws.iter().map(|d| d.0).collect();
    let proofs: Vec<[u8; 64]> = draws.iter().map(|d| d.1).collect();

    let hands: Vec<[Card; HAND_SIZE]> = preouts.iter().map(cards_from_preout).collect();
    let ranks: Vec<HandRank> = hands.iter().map(evaluate_hand).collect();
    let winner = decide_winner(&preouts, &ranks);

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

    for i in 0..n {
        verify_draw(i, &t.pubkeys[i], &seed, &t.preouts[i], &t.proofs[i])?;
    }

    let hands: Vec<[Card; HAND_SIZE]> = t.preouts.iter().map(cards_from_preout).collect();
    let ranks: Vec<HandRank> = hands.iter().map(evaluate_hand).collect();
    let winner = decide_winner(&t.preouts, &ranks);
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
        let preout = [42u8; 32];
        let a = cards_from_preout(&preout);
        let b = cards_from_preout(&preout);
        assert_eq!(a, b);
        for card in a {
            assert!((2..=14).contains(&card.rank));
            assert!(card.suit < 4);
        }
        assert_ne!(
            a,
            cards_from_preout(&[43u8; 32]),
            "different preout, different hand"
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
