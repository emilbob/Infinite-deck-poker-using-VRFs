//! JSON-in / JSON-out façade over the engine.
//!
//! This is the surface the browser talks to. It is deliberately plain Rust
//! with no wasm dependencies, so the exact payloads the UI consumes can be
//! tested natively; `wasm.rs` is only a thin `#[wasm_bindgen]` shim over it.
//!
//! Everything crosses the boundary as JSON strings rather than typed bindings.
//! [`Transcript`] already has a versioned wire format, so reusing it here keeps
//! one encoding for the browser, the disk, and any third-party verifier.

use crate::cheats::{deal_round, pick_cheat, Round, Tier};
use crate::{
    evaluate_hand, play_game, secret_from_passphrase, verify_transcript, Card, HandRank, Outcome,
    Player, Transcript,
};
use serde::Serialize;

/// A card, pre-rendered so the UI doesn't reimplement rank/suit formatting.
#[derive(Serialize)]
pub struct CardView {
    pub rank: u8,
    pub suit: u8,
    pub label: String,
}

impl From<&Card> for CardView {
    fn from(c: &Card) -> Self {
        Self {
            rank: c.rank,
            suit: c.suit,
            label: c.to_string(),
        }
    }
}

#[derive(Serialize)]
pub struct RankView {
    pub category: String,
    pub tiebreak: Vec<u8>,
}

impl From<&HandRank> for RankView {
    fn from(r: &HandRank) -> Self {
        Self {
            category: format!("{:?}", r.category),
            tiebreak: r.tiebreak.clone(),
        }
    }
}

/// An [`Outcome`] with bytes hex-encoded and hands pre-formatted.
#[derive(Serialize)]
pub struct OutcomeView {
    pub seed: String,
    pub hands: Vec<Vec<CardView>>,
    pub ranks: Vec<RankView>,
    pub winner: usize,
}

impl From<&Outcome> for OutcomeView {
    fn from(o: &Outcome) -> Self {
        Self {
            seed: hex::encode(o.seed),
            hands: o
                .hands
                .iter()
                .map(|h| h.iter().map(CardView::from).collect())
                .collect(),
            ranks: o.ranks.iter().map(RankView::from).collect(),
            winner: o.winner,
        }
    }
}

/// A freshly played game: the transcript to publish, and the outcome to render.
#[derive(Serialize)]
pub struct GameView {
    pub transcript: Transcript,
    pub outcome: OutcomeView,
    /// The transcript exactly as a third party would receive it. The UI shows
    /// this verbatim and lets the user edit it — see [`verify_json`].
    pub transcript_json: String,
}

/// The result of verifying a document, shaped so the UI can render either arm
/// without inspecting error strings.
#[derive(Serialize)]
pub struct VerifyView {
    pub ok: bool,
    pub error: Option<String>,
    pub outcome: Option<OutcomeView>,
}

/// Play a game of `players` seats, optionally seeding seat 0 from a passphrase.
///
/// Returns [`GameView`] as JSON.
pub fn deal_json(players: usize, passphrase: Option<&str>) -> String {
    let players = players.clamp(2, 10);
    let mut seats: Vec<Player> = (0..players).map(|_| Player::new()).collect();
    if let Some(p) = passphrase.filter(|p| !p.is_empty()) {
        seats[0].preset_secret(secret_from_passphrase(p));
    }

    let (transcript, outcome) = play_game(&mut seats);
    let view = GameView {
        transcript_json: transcript.to_json(),
        outcome: OutcomeView::from(&outcome),
        transcript,
    };
    serde_json::to_string(&view).expect("GameView is always serializable")
}

/// Verify a transcript document, exactly as a third party would.
///
/// Never returns an `Err`: a rejected transcript is a *result* the UI renders,
/// not an exception. Decode failures and verification failures both land in
/// `error`, which is what makes the tamper demo work.
pub fn verify_json(document: &str) -> String {
    let view = match Transcript::from_json(document).and_then(|t| verify_transcript(&t)) {
        Ok(outcome) => VerifyView {
            ok: true,
            error: None,
            outcome: Some(OutcomeView::from(&outcome)),
        },
        Err(e) => VerifyView {
            ok: false,
            error: Some(e.to_string()),
            outcome: None,
        },
    };
    serde_json::to_string(&view).expect("VerifyView is always serializable")
}

/// Evaluate an arbitrary 5-card hand — lets the UI explain hand rankings
/// without duplicating the evaluator in TypeScript.
pub fn evaluate_json(cards: &str) -> String {
    let parsed: Result<Vec<Card>, _> = serde_json::from_str(cards);
    let Ok(parsed) = parsed else {
        return r#"{"error":"expected an array of {rank,suit}"}"#.to_string();
    };
    let Ok(five): Result<[Card; 5], _> = parsed.try_into() else {
        return r#"{"error":"expected exactly 5 cards"}"#.to_string();
    };
    serde_json::to_string(&RankView::from(&evaluate_hand(&five)))
        .expect("RankView is always serializable")
}

// ---------------------------------------------------------------------------
// Catch the Cheat
// ---------------------------------------------------------------------------

/// A round as the player sees it *before* answering.
///
/// Deliberately carries no verdict, no nonce and no cheat — only the
/// commitment. The answer is withheld in [`Session`] rather than shipped and
/// hidden by the UI, so the commitment is a real promise instead of a prop.
#[derive(Serialize)]
pub struct RoundView {
    pub round: usize,
    pub total: usize,
    pub commitment: String,
    pub transcript_json: String,
    pub outcome: OutcomeView,
    pub score: usize,
    pub answered: usize,
}

/// What the player gets back once they have committed to an answer.
#[derive(Serialize)]
pub struct AnswerView {
    pub correct: bool,
    pub tampered: bool,
    pub cheat: String,
    pub tier: Option<Tier>,
    pub explanation: String,
    /// The verifier's own verdict, so the claim is never taken on trust.
    pub verifier_error: Option<String>,
    /// Opening of the commitment: the player can re-hash these and check.
    pub nonce: String,
    pub commitment: String,
    pub score: usize,
    pub answered: usize,
    pub total: usize,
    pub finished: bool,
}

const ROUNDS: usize = 10;

/// A ten-round run. Holds the pending round's answer so it cannot be read off
/// the wire before the player has committed to a guess.
#[derive(Default)]
pub struct Session {
    pending: Option<Round>,
    round: usize,
    score: usize,
    answered: usize,
}

impl Session {
    pub fn new() -> Self {
        Self::default()
    }

    /// Deal the next round. Re-dealing without answering forfeits nothing —
    /// the previous round is simply replaced, which also means a player cannot
    /// farm attempts at the same transcript.
    pub fn deal(&mut self, players: usize) -> String {
        let round = deal_round(players, pick_cheat(self.round));
        let view = RoundView {
            round: self.round,
            total: ROUNDS,
            commitment: hex::encode(round.commitment),
            transcript_json: round.transcript.to_json(),
            outcome: OutcomeView::from(&round.outcome),
            score: self.score,
            answered: self.answered,
        };
        self.pending = Some(round);
        serde_json::to_string(&view).expect("RoundView is always serializable")
    }

    /// Answer the pending round. `guess_tampered` is the player's verdict.
    ///
    /// Returns `null` if there is no pending round, rather than inventing one.
    pub fn answer(&mut self, guess_tampered: bool) -> String {
        let Some(round) = self.pending.take() else {
            return "null".to_string();
        };

        let tampered = round.is_tampered();
        let correct = guess_tampered == tampered;
        if correct {
            self.score += 1;
        }
        self.answered += 1;
        self.round += 1;

        let view = AnswerView {
            correct,
            tampered,
            cheat: round.cheat.label().to_string(),
            tier: round.cheat.tier(),
            explanation: round.cheat.explanation().to_string(),
            verifier_error: verify_transcript(&round.transcript)
                .err()
                .map(|e| e.to_string()),
            nonce: hex::encode(round.nonce),
            commitment: hex::encode(round.commitment),
            score: self.score,
            answered: self.answered,
            total: ROUNDS,
            finished: self.answered >= ROUNDS,
        };
        serde_json::to_string(&view).expect("AnswerView is always serializable")
    }

    pub fn reset(&mut self) {
        *self = Self::new();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    #[test]
    fn deal_payload_has_what_the_ui_needs() {
        let v: Value = serde_json::from_str(&deal_json(3, Some("hunter2"))).unwrap();
        assert_eq!(v["outcome"]["hands"].as_array().unwrap().len(), 3);
        assert_eq!(v["outcome"]["hands"][0].as_array().unwrap().len(), 5);
        assert!(v["outcome"]["hands"][0][0]["label"].as_str().unwrap().len() >= 2);
        assert_eq!(v["outcome"]["seed"].as_str().unwrap().len(), 64);
        assert!(v["transcript_json"]
            .as_str()
            .unwrap()
            .contains("\"version\""));
    }

    #[test]
    fn a_dealt_game_verifies_through_the_boundary() {
        let dealt: Value = serde_json::from_str(&deal_json(3, None)).unwrap();
        let document = dealt["transcript_json"].as_str().unwrap();

        let result: Value = serde_json::from_str(&verify_json(document)).unwrap();
        assert_eq!(result["ok"], true, "honest game must verify in the browser");
        assert_eq!(result["outcome"]["winner"], dealt["outcome"]["winner"]);
        assert_eq!(result["outcome"]["seed"], dealt["outcome"]["seed"]);
    }

    #[test]
    fn tampering_returns_a_rendered_error_not_a_panic() {
        let dealt: Value = serde_json::from_str(&deal_json(3, None)).unwrap();
        let document = dealt["transcript_json"].as_str().unwrap();

        // Flip a nibble inside the first reveal — the tamper demo's whole point.
        let reveal = dealt["transcript"]["reveals"][0].as_str().unwrap();
        let mut flipped = reveal.to_string();
        flipped.replace_range(0..1, if reveal.starts_with('0') { "1" } else { "0" });
        let tampered = document.replace(reveal, &flipped);

        let result: Value = serde_json::from_str(&verify_json(&tampered)).unwrap();
        assert_eq!(result["ok"], false);
        assert!(result["outcome"].is_null());
        assert!(result["error"]
            .as_str()
            .unwrap()
            .contains("does not match commitment"));
    }

    #[test]
    fn garbage_input_is_an_error_result_not_a_crash() {
        for junk in ["", "null", "{}", "not json", r#"{"version":99}"#] {
            let result: Value = serde_json::from_str(&verify_json(junk)).unwrap();
            assert_eq!(result["ok"], false, "junk input {junk:?} must not verify");
            assert!(result["error"].is_string());
        }
    }

    #[test]
    fn seat_count_is_clamped_to_something_playable() {
        let one: Value = serde_json::from_str(&deal_json(1, None)).unwrap();
        assert_eq!(one["outcome"]["hands"].as_array().unwrap().len(), 2);
        let many: Value = serde_json::from_str(&deal_json(500, None)).unwrap();
        assert_eq!(many["outcome"]["hands"].as_array().unwrap().len(), 10);
    }

    #[test]
    fn a_dealt_round_leaks_nothing_about_the_verdict() {
        // The whole point of holding the answer in Session: if any of these
        // appeared in the payload, the commitment would be theatre.
        for _ in 0..20 {
            let mut s = Session::new();
            let raw = s.deal(3);
            let v: Value = serde_json::from_str(&raw).unwrap();
            assert!(v.get("tampered").is_none(), "verdict leaked");
            assert!(v.get("cheat").is_none(), "cheat leaked");
            assert!(
                v.get("nonce").is_none(),
                "nonce leaked — commitment openable"
            );
            assert!(v.get("tier").is_none(), "tier leaked");
            assert!(v["commitment"].as_str().unwrap().len() == 64);
        }
    }

    #[test]
    fn the_commitment_opens_to_the_answer_given() {
        use crate::cheats::verdict_commitment;
        for guess in [true, false] {
            let mut s = Session::new();
            let dealt: Value = serde_json::from_str(&s.deal(3)).unwrap();
            let answered: Value = serde_json::from_str(&s.answer(guess)).unwrap();

            assert_eq!(
                dealt["commitment"], answered["commitment"],
                "the round must be judged against the commitment it published"
            );

            // Re-hash exactly as a suspicious player would.
            let nonce: [u8; 32] = hex::decode(answered["nonce"].as_str().unwrap())
                .unwrap()
                .try_into()
                .unwrap();
            let tampered = answered["tampered"].as_bool().unwrap();
            assert_eq!(
                hex::encode(verdict_commitment(tampered, &nonce)),
                answered["commitment"].as_str().unwrap(),
                "commitment must open to the verdict actually reported"
            );
        }
    }

    #[test]
    fn scoring_follows_the_verifier_not_the_label() {
        // `correct` must agree with what verify_transcript actually says, or the
        // game could mark a truthful player wrong.
        let mut s = Session::new();
        for _ in 0..30 {
            let _ = s.deal(3);
            let a: Value = serde_json::from_str(&s.answer(true)).unwrap();
            let tampered = a["tampered"].as_bool().unwrap();
            let rejected = !a["verifier_error"].is_null();
            assert_eq!(
                tampered, rejected,
                "declared verdict disagrees with the verifier: {a}"
            );
            assert_eq!(a["correct"].as_bool().unwrap(), tampered);
        }
    }

    #[test]
    fn a_run_finishes_after_ten_rounds() {
        let mut s = Session::new();
        let mut last = Value::Null;
        for i in 0..10 {
            let _ = s.deal(3);
            last = serde_json::from_str(&s.answer(false)).unwrap();
            assert_eq!(last["answered"], i + 1);
        }
        assert_eq!(last["finished"], true);
        assert!(last["score"].as_u64().unwrap() <= 10);
    }

    #[test]
    fn answering_without_a_round_is_null_not_a_guess() {
        let mut s = Session::new();
        assert_eq!(s.answer(true), "null");
        // And a round cannot be answered twice.
        let _ = s.deal(3);
        assert_ne!(s.answer(true), "null");
        assert_eq!(
            s.answer(true),
            "null",
            "the same round must not be re-scored"
        );
    }

    #[test]
    fn evaluate_json_explains_a_hand() {
        let five = r#"[{"rank":9,"suit":0},{"rank":9,"suit":1},{"rank":9,"suit":2},
                       {"rank":9,"suit":3},{"rank":9,"suit":0}]"#;
        let v: Value = serde_json::from_str(&evaluate_json(five)).unwrap();
        assert_eq!(v["category"], "FiveOfAKind");

        let bad: Value = serde_json::from_str(&evaluate_json("[]")).unwrap();
        assert!(bad["error"].is_string());
    }
}
