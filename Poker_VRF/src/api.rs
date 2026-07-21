//! JSON-in / JSON-out façade over the engine.
//!
//! This is the surface the browser talks to. It is deliberately plain Rust
//! with no wasm dependencies, so the exact payloads the UI consumes can be
//! tested natively; `wasm.rs` is only a thin `#[wasm_bindgen]` shim over it.
//!
//! Everything crosses the boundary as JSON strings rather than typed bindings.
//! [`Transcript`] already has a versioned wire format, so reusing it here keeps
//! one encoding for the browser, the disk, and any third-party verifier.

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
    fn evaluate_json_explains_a_hand() {
        let five = r#"[{"rank":9,"suit":0},{"rank":9,"suit":1},{"rank":9,"suit":2},
                       {"rank":9,"suit":3},{"rank":9,"suit":0}]"#;
        let v: Value = serde_json::from_str(&evaluate_json(five)).unwrap();
        assert_eq!(v["category"], "FiveOfAKind");

        let bad: Value = serde_json::from_str(&evaluate_json("[]")).unwrap();
        assert!(bad["error"].is_string());
    }
}
