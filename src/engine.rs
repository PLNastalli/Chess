//! Small, self-contained Rust chess engine. This is not Stockfish-strength.
use shakmaty::{Chess, Color, Move, Position, Role, Square, uci::UciMove};

const MATE: i32 = 100_000;
const INF: i32 = 1_000_000;

fn value(role: Role) -> i32 {
    match role {
        Role::Pawn => 100,
        Role::Knight => 320,
        Role::Bishop => 335,
        Role::Rook => 500,
        Role::Queen => 900,
        Role::King => 0,
    }
}

fn evaluate(position: &Chess) -> i32 {
    let mut score = 0;
    for square in Square::ALL {
        let Some(piece) = position.board().piece_at(square) else { continue };
        let file = square.to_u32() as i32 % 8;
        let rank = square.to_u32() as i32 / 8;
        let center = 6 - (file - 3).abs() - (rank - 3).abs();
        let bonus = match piece.role {
            Role::Pawn => if piece.color == Color::White { rank * 7 } else { (7 - rank) * 7 },
            Role::Knight | Role::Bishop => center * 5,
            _ => 0,
        };
        let v = value(piece.role) + bonus;
        score += if piece.color == position.turn() { v } else { -v };
    }
    score
}

fn ranked_moves(pos: &Chess) -> Vec<Move> {
    let mut moves: Vec<Move> = pos.legal_moves().into_iter().collect();
    moves.sort_by_key(|m| -(
        m.capture().map(value).unwrap_or(0) * 10
            + m.promotion().map(value).unwrap_or(0)
    ));
    moves
}

fn negamax(pos: &Chess, depth: u8, mut alpha: i32, beta: i32, ply: i32) -> i32 {
    if pos.is_checkmate() { return -MATE + ply; }
    if pos.is_stalemate() || pos.is_insufficient_material() { return 0; }
    if depth == 0 { return evaluate(pos); }
    let mut best = -INF;
    for mv in ranked_moves(pos) {
        let mut next = pos.clone();
        next.play_unchecked(mv);
        let score = -negamax(&next, depth - 1, -beta, -alpha, ply + 1);
        best = best.max(score);
        alpha = alpha.max(score);
        if alpha >= beta { break; }
    }
    best
}

/// Alpha-beta search; higher depths grow exponentially. Intended for depth 2-4.
pub fn best_move(pos: &Chess, depth: u8) -> Option<String> {
    let moves = ranked_moves(pos);
    let mut best = None;
    let mut score = -INF;
    let mut alpha = -INF;
    for mv in moves {
        let mut next = pos.clone();
        next.play_unchecked(mv);
        let current = -negamax(&next, depth.saturating_sub(1), -INF, -alpha, 1);
        if current > score {
            score = current;
            best = Some(UciMove::from_standard(mv).to_string());
        }
        alpha = alpha.max(current);
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opening_move_is_legal() {
        let position = Chess::default();
        let mv = best_move(&position, 2).unwrap();
        let uci: UciMove = mv.parse().unwrap();
        assert!(uci.to_move(&position).is_ok());
    }

    #[test]
    fn finds_forced_checkmate() {
        use shakmaty::fen::Fen;
        use shakmaty::CastlingMode;
        let fen: Fen = "7k/6pp/5Q2/8/8/8/8/6K1 w - - 0 1".parse().unwrap();
        let pos: Chess = fen.into_position(CastlingMode::Standard).unwrap();
        let mv = best_move(&pos, 2).unwrap();
        let mut next = pos.clone();
        next.play_unchecked(mv.parse::<UciMove>().unwrap().to_move(&pos).unwrap());
        assert!(next.is_checkmate());
    }
}
