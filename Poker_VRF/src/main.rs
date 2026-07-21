use poker_vrf::{play_game, secret_from_passphrase, verify_transcript, Player, Transcript};
use std::io::{self, Write};

fn main() {
    let n = 3;
    println!("♠ Verifiable infinite-deck poker — {n} players");
    println!("  you are player 1; players 2-{n} are simulated.\n");

    let mut players: Vec<Player> = (0..n).map(|_| Player::new()).collect();

    // The one place a human acts: contributing entropy to the shared seed.
    // Your cards are a function of (seed, your key), so nothing after this
    // point can change your hand — that is what makes the game verifiable.
    if let Some(passphrase) = prompt_passphrase() {
        players[0].preset_secret(secret_from_passphrase(&passphrase));
        println!("  → your contribution is derived from your passphrase.");
    } else {
        println!("  → using the system RNG for your contribution.");
    }

    let (transcript, outcome) = play_game(&mut players);

    println!(
        "\nyour secret  r_1 = {}",
        hex::encode(transcript.reveals[0])
    );
    println!(
        "your commit  c_1 = {}",
        hex::encode(transcript.commitments[0])
    );
    println!("shared seed  S   = {}", hex::encode(outcome.seed));
    println!("             S mixes every player's secret — no one steered it.\n");

    for (i, (hand, rank)) in outcome.hands.iter().zip(&outcome.ranks).enumerate() {
        let cards: Vec<String> = hand.iter().map(|c| c.to_string()).collect();
        let who = if i == 0 {
            "you".to_string()
        } else {
            format!("player {}", i + 1)
        };
        let marker = if i == outcome.winner {
            " ← winner"
        } else {
            ""
        };
        println!(
            "{:>8}: {}  ({:?}){}",
            who,
            cards.join(" "),
            rank.category,
            marker
        );
    }

    // Serialize, then verify only what came back off the wire — a third party
    // gets the JSON document and nothing else.
    let json = transcript.to_json();
    println!("\ntranscript: {} bytes of JSON", json.len());
    println!("re-verifying it as a third party…");

    match Transcript::from_json(&json).and_then(|t| verify_transcript(&t)) {
        Ok(v) => println!(
            "✔ transcript verifies: player {} wins with {:?}",
            v.winner + 1,
            v.ranks[v.winner].category
        ),
        Err(e) => println!("✘ transcript INVALID: {e}"),
    }

    println!(
        "\nnote: this is a protocol demo, not a fair game — one process holds\n\
         every player's secret, so whoever reveals last could steer the seed.\n\
         Real fairness needs a transport with reveal timeouts (see README)."
    );
}

/// Ask for a passphrase to derive the human player's secret. `None` means
/// "use the system RNG" (empty input, or a non-interactive stdin).
fn prompt_passphrase() -> Option<String> {
    print!("passphrase for your randomness (enter = system RNG): ");
    io::stdout().flush().ok()?;

    let mut line = String::new();
    if io::stdin().read_line(&mut line).ok()? == 0 {
        return None; // EOF: piped or redirected input
    }
    let trimmed = line.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}
