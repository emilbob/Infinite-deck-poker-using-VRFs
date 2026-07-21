//! Deliberately corrupted games, for the Catch the Cheat mode.
//!
//! Each [`Cheat`] produces a transcript that [`verify_transcript`] genuinely
//! rejects — nothing here fakes a failure. What varies is *how a human could
//! have caught it*: by reading the table, by doing arithmetic, or not at all.
//! That gradient ([`Tier`]) is the point of the mode.
//!
//! See `docs/m3-catch-the-cheat.md`.

use crate::{commitment_hash, play_game, Error, Player, Transcript};
use rand::rngs::OsRng;
use rand::RngCore;
use serde::Serialize;
use sha2::{Digest, Sha256};

const VERDICT_DOMAIN: &[u8] = b"poker-vrf.round-verdict.v1";

/// How a player could, in principle, have caught the tampering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Tier {
    /// Visible by reading the table.
    ByEye,
    /// Requires computing a hash — simple in principle, not by inspection.
    ByArithmetic,
    /// Undetectable without verifying the cryptography.
    Impossible,
}

/// A way to corrupt an otherwise honest game.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Cheat {
    /// An honest round. Not a cheat — the mode needs real negatives.
    None,
    /// Declare a winner who does not hold the best hand.
    SwappedWinner,
    /// Flip a byte of a revealed secret after committing to it.
    TamperedReveal,
    /// Publish a commitment computed under another player's key.
    StolenCommitment,
    /// Present another player's VRF proof as your own.
    ForgedProof,
    /// Exchange two players' VRF pre-outputs, so each claims the other's cards.
    SwappedPreouts,
}

impl Cheat {
    pub const ALL: [Cheat; 6] = [
        Cheat::None,
        Cheat::SwappedWinner,
        Cheat::TamperedReveal,
        Cheat::StolenCommitment,
        Cheat::ForgedProof,
        Cheat::SwappedPreouts,
    ];

    pub fn is_tampered(self) -> bool {
        self != Cheat::None
    }

    pub fn tier(self) -> Option<Tier> {
        match self {
            Cheat::None => None,
            Cheat::SwappedWinner => Some(Tier::ByEye),
            Cheat::TamperedReveal | Cheat::StolenCommitment => Some(Tier::ByArithmetic),
            Cheat::ForgedProof | Cheat::SwappedPreouts => Some(Tier::Impossible),
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Cheat::None => "Honest round",
            Cheat::SwappedWinner => "Winner swapped",
            Cheat::TamperedReveal => "Reveal tampered with",
            Cheat::StolenCommitment => "Commitment stolen from another player",
            Cheat::ForgedProof => "VRF proof forged",
            Cheat::SwappedPreouts => "Pre-outputs swapped between players",
        }
    }

    /// Shown after the player answers. States what happened and, crucially,
    /// whether they could have caught it — a wrong answer on an `Impossible`
    /// cheat should read as the lesson, not as a failure.
    pub fn explanation(self) -> &'static str {
        match self {
            Cheat::None => {
                "Nothing was altered. Every commitment, proof and hand recomputes, \
                 and the declared winner really does hold the best hand."
            }
            Cheat::SwappedWinner => {
                "The transcript names a winner who does not hold the best hand. \
                 This one you could have caught by reading the table — the cards \
                 are right there, and the claim contradicts them."
            }
            Cheat::TamperedReveal => {
                "A revealed secret was altered after it had been committed to, so it \
                 no longer hashes to its commitment. Catching this by eye is not \
                 possible: you would have had to hash the reveal yourself and \
                 compare. Simple arithmetic, but not something you can see."
            }
            Cheat::StolenCommitment => {
                "A player published a commitment computed under someone else's key. \
                 It is perfectly well-formed — it is simply not theirs. Commitments \
                 are bound to a public key precisely so this fails."
            }
            Cheat::ForgedProof => {
                "One player presented another player's VRF proof as their own. \
                 Nothing visible in the cards could have told you. A VRF proof is \
                 either valid for a given key and seed or it is not, and only the \
                 proof check can tell the difference."
            }
            Cheat::SwappedPreouts => {
                "Two players' pre-outputs were exchanged, so each claims the other's \
                 cards. The hands look entirely ordinary. Only checking each proof \
                 against the key that supposedly produced it catches this."
            }
        }
    }

    /// The error `verify_transcript` must return for this cheat.
    ///
    /// `None` for an honest round. The player index is fixed by [`apply`].
    pub fn expected_error(self) -> Option<Error> {
        match self {
            Cheat::None => None,
            Cheat::SwappedWinner => Some(Error::WrongWinner {
                claimed: 0,
                actual: 0,
            }),
            Cheat::TamperedReveal | Cheat::StolenCommitment => {
                Some(Error::CommitmentMismatch { player: 0 })
            }
            Cheat::ForgedProof | Cheat::SwappedPreouts => Some(Error::BadVrfProof { player: 0 }),
        }
    }

    /// Corrupt a transcript in place. Requires at least two players.
    fn apply(self, t: &mut Transcript) {
        match self {
            Cheat::None => {}
            Cheat::SwappedWinner => {
                t.winner = (t.winner + 1) % t.pubkeys.len();
            }
            Cheat::TamperedReveal => {
                t.reveals[0][0] ^= 0x01;
            }
            Cheat::StolenCommitment => {
                // Well-formed, just computed under player 1's key.
                t.commitments[0] = commitment_hash(&t.pubkeys[1], &t.reveals[0]);
            }
            Cheat::ForgedProof => {
                // A structurally valid proof that simply is not player 0's.
                t.proofs[0] = t.proofs[1];
            }
            Cheat::SwappedPreouts => {
                t.preouts.swap(0, 1);
            }
        }
    }
}

/// One round of Catch the Cheat.
///
/// `nonce` opens `commitment` — see [`verdict_commitment`]. Neither it nor
/// `cheat` may reach the player before they answer.
pub struct Round {
    pub transcript: Transcript,
    /// The table as shown to the player: the hands from the honest game, with
    /// the winner the *transcript* claims.
    ///
    /// A tampered transcript usually cannot produce a table at all — altering a
    /// reveal changes the seed, so no VRF proof verifies and no hand is
    /// derivable. Showing the honest hands under a corrupted transcript is also
    /// the realistic case: a cheating dealer shows you a plausible table and a
    /// transcript that does not back it up. It is what makes `SwappedWinner`
    /// catchable by eye and the rest invisible, which is exactly the tier split.
    pub outcome: crate::Outcome,
    pub cheat: Cheat,
    pub commitment: [u8; 32],
    pub nonce: [u8; 32],
}

impl Round {
    pub fn is_tampered(&self) -> bool {
        self.cheat.is_tampered()
    }
}

/// `H(domain ‖ verdict ‖ nonce)` — published before the player answers so the
/// game cannot change its mind afterwards, then opened once they have.
pub fn verdict_commitment(tampered: bool, nonce: &[u8; 32]) -> [u8; 32] {
    Sha256::new()
        .chain_update(VERDICT_DOMAIN)
        .chain_update([u8::from(tampered)])
        .chain_update(nonce)
        .finalize()
        .into()
}

/// Play an honest game, then apply `cheat` to its transcript.
pub fn deal_round(players: usize, cheat: Cheat) -> Round {
    let players = players.clamp(2, 10);
    let mut seats: Vec<Player> = (0..players).map(|_| Player::new()).collect();
    let (mut transcript, mut outcome) = play_game(&mut seats);
    cheat.apply(&mut transcript);
    // The table follows the transcript's claim, so a swapped winner shows up as
    // a marker on someone who plainly does not hold the best hand.
    outcome.winner = transcript.winner;

    let mut nonce = [0u8; 32];
    OsRng.fill_bytes(&mut nonce);

    Round {
        commitment: verdict_commitment(cheat.is_tampered(), &nonce),
        nonce,
        cheat,
        outcome,
        transcript,
    }
}

/// The cheats permitted at a given round number, forming the difficulty ladder:
/// by-eye only, then arithmetic, then everything.
pub fn pool_for_round(round: usize) -> &'static [Cheat] {
    match round {
        0..=2 => &[Cheat::SwappedWinner],
        3..=5 => &[
            Cheat::SwappedWinner,
            Cheat::TamperedReveal,
            Cheat::StolenCommitment,
        ],
        _ => &[
            Cheat::SwappedWinner,
            Cheat::TamperedReveal,
            Cheat::StolenCommitment,
            Cheat::ForgedProof,
            Cheat::SwappedPreouts,
        ],
    }
}

/// Choose a cheat for `round`, honest roughly two times in five.
///
/// Honest rounds have to be common enough that "always guess tampered" loses.
pub fn pick_cheat(round: usize) -> Cheat {
    let mut b = [0u8; 2];
    OsRng.fill_bytes(&mut b);
    if b[0] < 102 {
        return Cheat::None; // 102/256 ≈ 40%
    }
    let pool = pool_for_round(round);
    pool[b[1] as usize % pool.len()]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::verify_transcript;

    /// The load-bearing test for this whole mode: every cheat must genuinely
    /// fail verification, with the error the UI is going to explain. A game
    /// about detecting dishonesty must not be wrong about the answer.
    #[test]
    fn every_cheat_is_rejected_with_its_expected_error() {
        for cheat in Cheat::ALL {
            let round = deal_round(4, cheat);
            let got = verify_transcript(&round.transcript);

            match (cheat.expected_error(), got) {
                (None, Ok(_)) => {}
                (None, Err(e)) => panic!("{cheat:?} is honest but was rejected: {e}"),
                (Some(_), Ok(_)) => panic!("{cheat:?} was accepted — the mode would lie"),
                (Some(expected), Err(actual)) => assert_eq!(
                    std::mem::discriminant(&expected),
                    std::mem::discriminant(&actual),
                    "{cheat:?}: expected {expected:?}, got {actual:?}"
                ),
            }
        }
    }

    #[test]
    fn honest_rounds_verify_repeatedly() {
        // Guards against a cheat that only sometimes bites — e.g. SwappedWinner
        // landing on a genuine tie, or a random hand making it a no-op.
        for _ in 0..25 {
            let round = deal_round(3, Cheat::None);
            assert!(!round.is_tampered());
            assert!(verify_transcript(&round.transcript).is_ok());
        }
    }

    #[test]
    fn tampered_rounds_never_verify() {
        for cheat in Cheat::ALL.into_iter().filter(|c| c.is_tampered()) {
            for _ in 0..10 {
                let round = deal_round(3, cheat);
                assert!(
                    verify_transcript(&round.transcript).is_err(),
                    "{cheat:?} verified on some deal — the mode would call a \
                     tampered round honest"
                );
            }
        }
    }

    #[test]
    fn verdict_commitment_opens_and_binds() {
        let nonce = [7u8; 32];
        let c = verdict_commitment(true, &nonce);
        assert_eq!(c, verdict_commitment(true, &nonce), "must be deterministic");
        assert_ne!(c, verdict_commitment(false, &nonce), "verdict must bind");
        assert_ne!(c, verdict_commitment(true, &[8u8; 32]), "nonce must bind");
    }

    #[test]
    fn a_round_commitment_matches_its_verdict() {
        for cheat in Cheat::ALL {
            let r = deal_round(3, cheat);
            assert_eq!(
                r.commitment,
                verdict_commitment(r.is_tampered(), &r.nonce),
                "{cheat:?}: published commitment must open to the real verdict"
            );
        }
    }

    #[test]
    fn a_swapped_winner_is_visible_in_the_table() {
        // The by-eye tier only works if the shown table actually contradicts the
        // claim. Over many deals the marked winner must not hold the best hand.
        let mut contradicted = 0;
        for _ in 0..25 {
            let r = deal_round(3, Cheat::SwappedWinner);
            let best = r
                .outcome
                .ranks
                .iter()
                .enumerate()
                .max_by(|a, b| a.1.cmp(b.1))
                .map(|(i, _)| i)
                .unwrap();
            if r.outcome.winner != best {
                contradicted += 1;
            }
        }
        assert!(
            contradicted >= 23,
            "a swapped winner must visibly contradict the table; only {contradicted}/25 did"
        );
    }

    #[test]
    fn an_honest_table_agrees_with_its_claim() {
        for _ in 0..25 {
            let r = deal_round(3, Cheat::None);
            let best = r
                .outcome
                .ranks
                .iter()
                .enumerate()
                .max_by(|a, b| a.1.cmp(b.1))
                .map(|(i, _)| i)
                .unwrap();
            assert_eq!(
                r.outcome.ranks[r.outcome.winner], r.outcome.ranks[best],
                "an honest round must not look like a swapped winner"
            );
        }
    }

    #[test]
    fn tiers_match_detectability() {
        assert_eq!(Cheat::None.tier(), None);
        assert_eq!(Cheat::SwappedWinner.tier(), Some(Tier::ByEye));
        assert_eq!(Cheat::ForgedProof.tier(), Some(Tier::Impossible));
        for cheat in Cheat::ALL.into_iter().filter(|c| c.is_tampered()) {
            assert!(cheat.tier().is_some(), "{cheat:?} must have a tier");
        }
    }

    #[test]
    fn difficulty_ladder_widens() {
        assert_eq!(pool_for_round(0), &[Cheat::SwappedWinner]);
        assert!(pool_for_round(0)
            .iter()
            .all(|c| c.tier() == Some(Tier::ByEye)));
        assert!(pool_for_round(4).len() > pool_for_round(0).len());
        assert!(pool_for_round(9)
            .iter()
            .any(|c| c.tier() == Some(Tier::Impossible)));
    }

    #[test]
    fn early_rounds_are_never_impossible() {
        // A Tier-3 cheat in round 1 would teach the wrong lesson first.
        for round in 0..3 {
            for _ in 0..50 {
                let c = pick_cheat(round);
                assert_ne!(c.tier(), Some(Tier::Impossible), "round {round} gave {c:?}");
            }
        }
    }

    #[test]
    fn honest_rounds_are_common_enough_to_matter() {
        // If honest rounds were rare, "always guess tampered" would win.
        let honest = (0..400)
            .filter(|&i| pick_cheat(i % 10) == Cheat::None)
            .count();
        assert!(
            (60..=260).contains(&honest),
            "expected a meaningful share of honest rounds, got {honest}/400"
        );
    }
}
