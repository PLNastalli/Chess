use shakmaty::{
    Chess, Color, Position, Role, Square, san::SanPlus, uci::UciMove,
};

#[derive(Clone)]
pub struct Game {
    pub position: Chess,
    pub moves: Vec<String>,
    pub notation: Vec<String>,
    pub last: Option<(Square, Square)>,
    positions: Vec<Chess>,
}

impl Default for Game {
    fn default() -> Self {
        let position = Chess::default();
        Self {
            positions: vec![position.clone()],
            position,
            moves: Vec::new(),
            notation: Vec::new(),
            last: None,
        }
    }
}

impl Game {
    pub fn turn(&self) -> Color {
        self.position.turn()
    }

    pub fn legal_targets(&self, square: Square) -> Vec<Square> {
        self.position
            .legal_moves()
            .into_iter()
            .filter_map(|m| {
                let uci = UciMove::from_standard(m);
                (uci.from() == Some(square)).then(|| uci.to()).flatten()
            })
            .collect()
    }

    pub fn make_move(&mut self, uci: &str) -> Result<(), String> {
        if self.is_finished() {
            return Err("A partida já terminou.".into());
        }
        let parsed: UciMove = uci.parse().map_err(|_| "Notação de lance inválida")?;
        let mv = parsed
            .to_move(&self.position)
            .map_err(|_| "Lance ilegal para a posição atual")?;
        if !self.position.legal_moves().contains(&mv) {
            return Err("Lance ilegal".into());
        }
        let standardized = UciMove::from_standard(mv);
        let from = standardized.from().ok_or("Origem inválida")?;
        let to = standardized.to().ok_or("Destino inválido")?;
        let san = SanPlus::from_move_and_play_unchecked(&mut self.position, mv);
        self.last = Some((from, to));
        self.moves.push(standardized.to_string());
        self.notation.push(san.to_string());
        self.positions.push(self.position.clone());
        Ok(())
    }

    pub fn is_finished(&self) -> bool {
        self.position.is_checkmate()
            || self.position.is_stalemate()
            || self.position.is_insufficient_material()
            || self.repetitions() >= 3
            || self.position.halfmoves() >= 100
    }

    pub fn repetitions(&self) -> usize {
        self.positions
            .iter()
            .filter(|p| *p == &self.position)
            .count()
    }

    pub fn status(&self) -> String {
        if self.position.is_checkmate() {
            return match self.turn() {
                Color::White => "Xeque-mate! Pretas venceram".into(),
                Color::Black => "Xeque-mate! Brancas venceram".into(),
            };
        }
        if self.position.is_stalemate() {
            return "Empate por afogamento".into();
        }
        if self.position.is_insufficient_material() {
            return "Empate: material insuficiente".into();
        }
        if self.repetitions() >= 3 {
            return "Empate por tripla repetição".into();
        }
        if self.position.halfmoves() >= 100 {
            return "Empate pela regra dos 50 lances".into();
        }
        let turn = if self.turn() == Color::White { "Brancas" } else { "Pretas" };
        if self.position.is_check() {
            format!("{turn} em xeque")
        } else {
            format("Vez das {turn}")
        }
    }

    pub fn piece_glyph(&self, square: Square) -> &'static str {
        let Some(piece) = self.position.board().piece_at(square) else {
            return "";
        };
        match (piece.color, piece.role) {
            (Color::White, Role::King) => "♔",
            (Color::White, Role::Queen) => "♕",
            (Color::White, Role::Rook) => "♖",
            (Color::White, Role::Bishop) => "♗",
            (Color::White, Role::Knight) => "♘",
            (Color::White, Role::Pawn) => "♙",
            (Color::Black, Role::King) => "♚",
            (Color::Black, Role::Queen) => "♛",
            (Color::Black, Role::Rook) => "♜",
            (Color::Black, Role::Bishop) => "♝",
            (Color::Black, Role::Knight) => "♞",
            (Color::Black, Role::Pawn) => "♟",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opening_has_twenty_legal_moves() {
        assert_eq!(Chess::default().legal_moves().len(), 20);
    }

    #[test]
    fn rejects_illegal_move() {
        let mut game = Game::default();
        assert!(game.make_move("e2e5").is_err());
        assert_eq!(game.moves.len(), 0);
    }

    #[test]
    fn records_scholars_mate() {
        let mut game = Game::default();
        for mv in ["e2e4", "e7e5", "d1h5", "b8c6", "f1c4", "g8f6", "h5f7"] {
            game.make_move(mv).unwrap();
        }
        assert!(game.position.is_checkmate());
        assert!(game.status().contains("Brancas venceram"));
    }

    #[test]
    fn supports_castling_and_queenside_rook() {
        let mut game = Game::default();
        for mv in ["e2e4", "e7e5", "g1f3", "b8c6", "f1e2", "g8f6", "e1g1"] {
            game.make_move(mv).unwrap();
        }
        assert_eq!(game.position.board().piece_at(Square::G1).unwrap().role, Role::King);
        assert_eq!(game.position.board().piece_at(Square::F1).unwrap().role, Role::Rook);
    }

    #[test]
    fn threefold_repetition_is_draw() {
        let mut game = Game::default();
        for mv in ["g1f3","g8f6","f3g1","f6g8","g1f3","g8f6","f3g1","f6g8"] {
            game.make_move(mv).unwrap();
        }
        assert!(game.is_finished());
    }
}
