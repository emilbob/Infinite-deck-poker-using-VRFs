//! `#[wasm_bindgen]` shim over [`crate::api`].
//!
//! Intentionally logic-free — every function here forwards to a plain-Rust
//! counterpart in `api.rs` that is covered by native tests. Anything that
//! needs testing belongs there, not here.

use crate::api;
use std::cell::RefCell;
use wasm_bindgen::prelude::*;

/// Play a game and return a `GameView` as JSON.
///
/// `passphrase` may be empty, in which case seat 0 uses the system RNG
/// (`getrandom/js` — the browser's `crypto.getRandomValues`).
#[wasm_bindgen]
pub fn deal(players: usize, passphrase: &str) -> String {
    api::deal_json(players, Some(passphrase))
}

/// Verify a transcript document, returning a `VerifyView` as JSON.
///
/// A rejected transcript is a normal return value, not a thrown error — the
/// tamper demo depends on being able to render the failure.
#[wasm_bindgen]
pub fn verify(document: &str) -> String {
    api::verify_json(document)
}

/// Evaluate an arbitrary 5-card hand, returning a `RankView` as JSON.
#[wasm_bindgen]
pub fn evaluate(cards: &str) -> String {
    api::evaluate_json(cards)
}

/// The transcript wire version this build reads.
#[wasm_bindgen]
pub fn transcript_version() -> u32 {
    crate::TRANSCRIPT_VERSION
}

// ---------------------------------------------------------------------------
// Catch the Cheat
// ---------------------------------------------------------------------------

// One run per tab. The session lives here rather than in JS specifically so the
// round's verdict never crosses the boundary before the player has answered —
// that is what makes the published commitment a promise instead of a prop.
thread_local! {
    static SESSION: RefCell<api::Session> = RefCell::new(api::Session::new());
}

/// Deal the next round. Returns a `RoundView` as JSON — no verdict in it.
#[wasm_bindgen]
pub fn round_deal(players: usize) -> String {
    SESSION.with(|s| s.borrow_mut().deal(players))
}

/// Answer the pending round; `true` means "this one was tampered with".
///
/// Returns an `AnswerView` as JSON, or `null` if no round is pending.
#[wasm_bindgen]
pub fn round_answer(guess_tampered: bool) -> String {
    SESSION.with(|s| s.borrow_mut().answer(guess_tampered))
}

/// Start a fresh run.
#[wasm_bindgen]
pub fn round_reset() {
    SESSION.with(|s| s.borrow_mut().reset())
}
