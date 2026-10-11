//! Standalone UCI command-line chess engine. GUI applications can launch
//! `rookery-engine` as a subprocess and talk UCI over stdin/stdout.
#[path = "../engine.rs"]
mod engine;

use shakmaty::{CastlingMode, Chess, Position, fen::Fen, uci::UciMove};
use std::io::{self, BufRead};

fn position_from_command(command: &str) -> Result<Chess, String> {
    let input = command.trim().strip_prefix("position ")
        .ok_or("expected position command")?;
    let (setup, moves) = input.split_once(" moves ").unwrap_or((input, ""));
    let mut position = if setup == "startpos" {
        Chess::default()
    } else if let Some(fen) = setup.strip_prefix("fen ") {
        let fen: Fen = fen.parse().map_err(|_| "invalid FEN")?;
        fen.into_position(CastlingMode::Standard)
            .map_err(|_| "invalid chess position")?
    } else {
        return Err("expected 'startpos' or 'fen <FEN>'".into());
    };

    for notation in moves.split_whitespace() {
        let uci: UciMove = notation.parse().map_err(|_| "invalid UCI move")?;
        let legal = uci.to_move(&position).map_err(|_| "illegal UCI move")?;
        position.play_unchecked(legal);
    }
    Ok(position)
}

fn depth_from_command(command: &str) -> u8 {
    let tokens: Vec<_> = command.split_whitespace().collect();
    tokens.windows(2)
        .find_map(|words| (words[0] == "depth").then(|| words[1].parse::<u8>().ok()).flatten())
        .unwrap_or(3)
        .clamp(1, 6)
}

fn main() {
    let stdin = io::stdin();
    let mut position = Chess::default();
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        let command = line.trim();
        match command {
            "uci" => {
                println!("id name Rookery");
                println!("id author PLNastalli");
                println!("uciok");
            }
            "isready" => println!("readyok"),
            "ucinewgame" => position = Chess::default(),
            "quit" => break,
            _ if command.starts_with("position ") => {
                match position_from_command(command) {
                    Ok(parsed) => position = parsed,
                    Err(message) => println!("info string {message}"),
                }
            }
            _ if command == "go" || command.starts_with("go ") => {
                let depth = depth_from_command(command);
                println!("info string searching depth {depth}");
                let move_to_play = engine::best_move(&position, depth);
                println!("bestmove {}", move_to_play.as_deref().unwrap_or("0000"));
            }
            "stop" => {}
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shakmaty::{Role, Square};

    #[test]
    fn parses_startpos_moves() {
        let pos = position_from_command("position startpos moves e2e4 e7e5 g1f3").unwrap();
        assert_eq!(pos.turn(), shakmaty::Color::Black);
        assert_eq!(pos.board().piece_at(Square::F3).unwrap().role, Role::Knight);
    }

    #[test]
    fn rejects_bad_position_without_mutating_existing_one() {
        assert!(position_from_command("position startpos moves e2e5").is_err());
        assert!(position_from_command("position fen invalid").is_err());
    }

    #[test]
    fn supports_valid_fen() {
        let pos = position_from_command("position fen 7k/6pp/5Q2/8/8/8/8/6K1 w - - 0 1").unwrap();
        assert_eq!(pos.board().piece_at(Square::F6).unwrap().role, Role::Queen);
    }

    #[test]
    fn reads_depth_and_caps_cost() {
        assert_eq!(depth_from_command("go depth 2"), 2);
        assert_eq!(depth_from_command("go movetime 500"), 3);
        assert_eq!(depth_from_command("go depth 99"), 6);
    }
}
