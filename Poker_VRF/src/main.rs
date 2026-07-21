use poker_vrf::{play_game, verify_transcript, Player};

fn main() {
    let n = 3;
    println!("♠ Verifiable infinite-deck poker — {n} players\n");

    let mut players: Vec<Player> = (0..n).map(|_| Player::new()).collect();
    let (transcript, outcome) = play_game(&mut players);

    println!("shared seed: {}", hex(&outcome.seed));
    println!();
    for (i, (hand, rank)) in outcome.hands.iter().zip(&outcome.ranks).enumerate() {
        let cards: Vec<String> = hand.iter().map(|c| c.to_string()).collect();
        let marker = if i == outcome.winner {
            " ← winner"
        } else {
            ""
        };
        println!(
            "player {}: {}  ({:?}){}",
            i + 1,
            cards.join(" "),
            rank.category,
            marker
        );
    }

    println!("\nre-verifying transcript as a third party…");
    match verify_transcript(&transcript) {
        Ok(v) => println!(
            "✔ transcript verifies: player {} wins with {:?}",
            v.winner + 1,
            v.ranks[v.winner].category
        ),
        Err(e) => println!("✘ transcript INVALID: {e}"),
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
