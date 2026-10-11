#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

mod engine;
mod game;
mod network;
mod ui;

use gpui_kit::component::{
    button::{Button, ButtonVariants},
    input::{Input, InputEvent, InputState},
};
use gpui_kit::*;
use shakmaty::{Color, Position, Role, Square};
use std::{collections::VecDeque, sync::mpsc::{self, Receiver}, time::Duration};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Local,
    Computer,
    P2p,
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum AppTab { Play, Puzzles, Learn, Train, Watch, Community, Other }
#[derive(Clone, Copy, PartialEq, Eq)]
enum BoardInteraction { Play, Puzzle, Preview }
const PUZZLES: [(&str, &str); 2] = [
    ("7k/6pp/5Q2/8/8/8/8/6K1 w - - 0 1", "f6f8"),
    ("7k/5K2/6Q1/8/8/8/8/8 w - - 0 1", "g6g7"),
];
const STUDY_MOVES: [&str; 16] = [
    "e2e4", "e7e5", "g1f3", "b8c6", "f1b5", "a7a6",
    "b5a4", "g8f6", "e1g1", "f8e7", "f1e1", "b7b5",
    "a4b3", "d7d6", "c2c3", "e8g8",
];

#[derive(Clone)]
struct ChatEntry {
    mine: bool,
    text: String,
}

const MAX_CHAT_HISTORY: usize = 100;

struct ChessApp {
    game: game::Game,
    tab: AppTab,
    search_input: Entity<InputState>,
    puzzle_game: game::Game,
    puzzle_selected: Option<Square>,
    puzzle_index: usize,
    puzzle_solved: bool,
    puzzle_notice: String,
    lesson_index: usize,
    study_game: game::Game,
    study_ply: usize,
    engine_depth: u8,
    flip_board: bool,
    selected: Option<Square>,
    mode: Mode,
    player: Color,
    connected: bool,
    peer: Option<network::Peer>,
    engine_result: Option<Receiver<Option<String>>>,
    address: Entity<InputState>,
    chat_input: Entity<InputState>,
    _chat_subscription: Subscription,
    chat_messages: VecDeque<ChatEntry>,
    notice: String,
    promotion: Role,
    game_over: Option<String>,
}

impl ChessApp {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let chat_input = cx.new(|cx| {
            InputState::new(window, cx).placeholder("Escreva uma mensagem...")
        });
        let chat_subscription = cx.subscribe_in(
            &chat_input,
            window,
            |this, _, event: &InputEvent, window, cx| {
                if matches!(event, InputEvent::PressEnter { .. }) {
                    this.send_chat(window, cx);
                }
            },
        );
        Self {
            game: game::Game::default(),
            tab: AppTab::Play,
            search_input: cx.new(|cx| InputState::new(window, cx).placeholder("Buscar aba...")),
            puzzle_game: game::Game::from_fen(PUZZLES[0].0).expect("valid puzzle FEN"),
            puzzle_selected: None,
            puzzle_index: 0,
            puzzle_solved: false,
            puzzle_notice: "Selecione uma peça e encontre o mate.".into(),
            lesson_index: 0,
            study_game: game::Game::default(),
            study_ply: 0,
            engine_depth: 3,
            flip_board: false,
            selected: None,
            mode: Mode::Local,
            player: Color::White,
            connected: false,
            peer: None,
            engine_result: None,
            address: cx.new(|cx| {
                InputState::new(window, cx)
                    .placeholder("IPv4 do anfitrião:50555")
            }),
            chat_input,
            _chat_subscription: chat_subscription,
            chat_messages: VecDeque::new(),
            notice: "Escolha o modo de jogo.".into(),
            promotion: Role::Queen,
            game_over: None,
        }
    }

    fn start_poll(&mut self, cx: &mut Context<Self>) {
        cx.spawn(async move |weak, cx| {
            loop {
                cx.background_executor().timer(Duration::from_millis(70)).await;
                if weak.update(cx, |app, cx| app.poll(cx)).is_err() {
                    break;
                }
            }
        }).detach();
    }

    fn poll(&mut self, cx: &mut Context<Self>) {
        let events: Vec<_> = self.peer.as_ref().map(|p| p.incoming.try_iter().take(32).collect()).unwrap_or_default();
        for event in events {
            match event {
                network::Event::Listening(_bind) => {
                    self.notice = format!("Hospedagem na porta TCP {}. Use ipconfig para achar seu IPv4 e compartilhe IP:50555; não use 0.0.0.0.", network::PORT);
                }
                network::Event::Connected => {
                    self.connected = true;
                    self.notice = "Conectado! Partida P2P iniciada.".into();
                }
                network::Event::Message(network::Message::Move { ply, uci }) => {
                    if self.game_over.is_some() { continue; }
                    if !self.connected || self.game.is_finished() || self.game.moves.len() != ply
                        || self.game.turn() == self.player || self.game.make_move(&uci).is_err() {
                        self.notice = "Adversário enviou um lance inválido. Partida encerrada.".into();
                        self.connected = false;
                        self.peer = None;
                    } else {
                        self.selected = None;
                        self.notice = format!("Adversário jogou {uci}");
                    }
                }
                network::Event::Message(network::Message::Chat { text }) => {
                    if self.mode == Mode::P2p && self.connected {
                        self.add_chat_message(false, text);
                    }
                }
                network::Event::Message(network::Message::Resign) => {
                    self.notice = "Adversário desistiu.".into();
                    self.game_over = Some("Você venceu por desistência".into());
                    self.connected = false;
                }
                network::Event::Message(network::Message::Hello { .. }) => {}
                network::Event::Disconnected => {
                    if self.connected && self.game_over.is_none() {
                        self.notice = "Adversário desconectou.".into();
                        self.connected = false;
                        self.peer = None;
                    }
                }
                network::Event::Error(message) => {
                    self.notice = message;
                    self.connected = false;
                    self.peer = None;
                }
            }
            cx.notify();
        }

        let result = self.engine_result.as_ref().and_then(|rx| rx.try_recv().ok());
        if let Some(best) = result {
            self.engine_result = None;
            if self.mode == Mode::Computer
                && self.game.turn() == Color::Black
                && let Some(mv) = best
                && self.game.make_move(&mv).is_ok()
            {
                self.notice = format!("Motor jogou {mv}");
            }
            cx.notify();
        }
    }

    fn reset(&mut self, mode: Mode, cx: &mut Context<Self>) {
        self.game = game::Game::default();
        self.selected = None;
        self.mode = mode;
        self.player = Color::White;
        self.connected = false;
        self.peer = None;
        self.engine_result = None;
        self.game_over = None;
        self.chat_messages.clear();
        self.notice = match mode {
            Mode::Local => "Partida local. Brancas começam.".into(),
            Mode::Computer => "Você joga de brancas contra o motor.".into(),
            Mode::P2p => "Configurando conexão P2P...".into(),
        };
        cx.notify();
    }

    fn add_chat_message(&mut self, mine: bool, text: String) {
        if self.chat_messages.len() == MAX_CHAT_HISTORY {
            self.chat_messages.pop_front();
        }
        self.chat_messages.push_back(ChatEntry { mine, text });
    }

    fn send_chat(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.mode != Mode::P2p || !self.connected {
            self.notice = "Conecte-se a um adversário para usar o chat.".into();
            cx.notify();
            return;
        }
        let draft = self.chat_input.read(cx).value().to_string();
        let Some(text) = network::normalize_chat_text(&draft) else {
            self.notice = format!(
                "Digite uma mensagem de até {} caracteres.",
                network::MAX_CHAT_CHARS
            );
            cx.notify();
            return;
        };
        let result = self.peer.as_ref().ok_or_else(|| "Conexão P2P indisponível.".to_string())
            .and_then(|peer| peer.send(network::Message::Chat { text: text.clone() }));
        match result {
            Ok(()) => {
                self.add_chat_message(true, text);
                self.chat_input.update(cx, |state, cx| state.set_value("", window, cx));
            }
            Err(error) => self.notice = error,
        }
        cx.notify();
    }

    fn host(&mut self, cx: &mut Context<Self>) {
        self.reset(Mode::P2p, cx);
        self.peer = Some(network::Peer::host());
        self.notice = format!("Hospedando na porta TCP {}...", network::PORT);
        cx.notify();
    }

    fn join(&mut self, cx: &mut Context<Self>) {
        let address = self.address.read(cx).value().trim().to_string();
        if address.is_empty() {
            self.notice = "Informe o IP do anfitrião:50555. O anfitrião deve clicar em Hospedar primeiro.".into();
            cx.notify();
            return;
        }
        self.reset(Mode::P2p, cx);
        self.player = Color::Black;
        self.peer = Some(network::Peer::join(address));
        self.notice = "Conectando ao anfitrião...".into();
        cx.notify();
    }

    fn undo(&mut self, cx: &mut Context<Self>) {
        if self.mode == Mode::P2p {
            self.notice = "Desfazer não está disponível no multiplayer.".into();
        } else {
            let waiting = self.engine_result.take().is_some();
            let count = if self.mode == Mode::Computer && !waiting && self.game.moves.len().is_multiple_of(2) {
                2
            } else {
                1
            };
            let mut reverted = false;
            for _ in 0..count { reverted |= self.game.undo(); }
            self.selected = None;
            self.game_over = None;
            self.notice = if reverted { "Último lance desfeito.".into() } else { "Nenhum lance para desfazer.".into() };
        }
        cx.notify();
    }

    fn resign(&mut self, cx: &mut Context<Self>) {
        if self.mode != Mode::P2p || !self.connected || self.game_over.is_some() {
            self.notice = "Não há uma partida P2P ativa.".into();
        } else if let Some(peer) = &self.peer {
            match peer.send(network::Message::Resign) {
                Ok(()) => {
                    self.game_over = Some("Você desistiu".into());
                    self.connected = false;
                    self.notice = "Desistência enviada ao adversário.".into();
                }
                Err(error) => self.notice = error,
            }
        }
        cx.notify();
    }

    fn schedule_engine(&mut self) {
        let position = self.game.position.clone();
        let depth = self.engine_depth;
        let (tx, rx) = mpsc::channel();
        self.engine_result = Some(rx);
        std::thread::spawn(move || {
            let result = engine::best_move(&position, depth);
            let _ = tx.send(result);
        });
    }

    fn select_square(&mut self, square: Square, cx: &mut Context<Self>) {
        if self.game.is_finished() || self.game_over.is_some() || self.engine_result.is_some() {
            return;
        }
        if self.mode == Mode::P2p && (!self.connected || self.game.turn() != self.player) {
            return;
        }
        let piece = self.game.position.board().piece_at(square);
        if let Some(from) = self.selected {
            if from == square {
                self.selected = None;
                cx.notify();
                return;
            }
            if piece.is_some_and(|p| p.color == self.game.turn()) {
                self.selected = Some(square);
                cx.notify();
                return;
            }
            let promotion = self.game.position.board().piece_at(from).is_some_and(|p|
                p.role == Role::Pawn && (square.rank() == shakmaty::Rank::Eighth
                    || square.rank() == shakmaty::Rank::First)
            );
            let promotion_suffix = if promotion {
                match self.promotion {
                    Role::Queen => "q",
                    Role::Rook => "r",
                    Role::Bishop => "b",
                    Role::Knight => "n",
                    _ => "q",
                }
            } else { "" };
            let uci = format!("{from}{square}{promotion_suffix}");
            let ply = self.game.moves.len();
            match self.game.make_move(&uci) {
                Ok(()) => {
                    self.selected = None;
                    self.notice.clear();
                    if self.mode == Mode::P2p {
                        if let Some(peer) = &self.peer
                            && let Err(error) = peer.send(network::Message::Move { ply, uci })
                        {
                            self.notice = error;
                            self.connected = false;
                        }
                    } else if self.mode == Mode::Computer && !self.game.is_finished() {
                        self.schedule_engine();
                        self.notice = "Motor calculando lance...".into();
                    }
                }
                Err(_) => {
                    self.selected = None;
                    self.notice = "Lance ilegal.".into();
                }
            }
        } else if piece.is_some_and(|p| p.color == self.game.turn()) {
            self.selected = Some(square);
        }
        cx.notify();
    }


    fn select_puzzle_square(&mut self, square: Square, cx: &mut Context<Self>) {
        if self.puzzle_solved {
            return;
        }
        let piece = self.puzzle_game.position.board().piece_at(square);
        match self.puzzle_selected {
            Some(from) if from == square => self.puzzle_selected = None,
            Some(_) if piece.is_some_and(|p| p.color == self.puzzle_game.turn()) => {
                self.puzzle_selected = Some(square);
            }
            Some(from) => {
                let uci = format!("{from}{square}");
                if uci == PUZZLES[self.puzzle_index].1
                    && self.puzzle_game.make_move(&uci).is_ok()
                {
                    self.puzzle_solved = true;
                    self.puzzle_notice = "Correto! Mate encontrado.".into();
                } else {
                    self.puzzle_notice = "Esse lance não é o mate. Tente outra vez.".into();
                }
                self.puzzle_selected = None;
            }
            None if piece.is_some_and(|p| p.color == self.puzzle_game.turn()) => {
                self.puzzle_selected = Some(square);
            }
            None => {}
        }
        cx.notify();
    }

    fn restart_puzzle(&mut self, cx: &mut Context<Self>) {
        self.puzzle_game = game::Game::from_fen(PUZZLES[self.puzzle_index].0)
            .expect("bundled puzzle FEN valid");
        self.puzzle_selected = None;
        self.puzzle_solved = false;
        self.puzzle_notice = "Encontre o mate em um lance.".into();
        cx.notify();
    }

    fn next_puzzle(&mut self, cx: &mut Context<Self>) {
        self.puzzle_index = (self.puzzle_index + 1) % PUZZLES.len();
        self.restart_puzzle(cx);
    }

    fn study_step(&mut self, forward: bool, cx: &mut Context<Self>) {
        self.study_ply = if forward {
            (self.study_ply + 1).min(STUDY_MOVES.len())
        } else {
            self.study_ply.saturating_sub(1)
        };
        self.study_game = game::Game::default();
        for mv in STUDY_MOVES.iter().take(self.study_ply) {
            if self.study_game.make_move(mv).is_err() {
                self.study_ply = 0;
                self.study_game = game::Game::default();
                break;
            }
        }
        cx.notify();
    }

    fn search_sections(&mut self, cx: &mut Context<Self>) {
        let q = self.search_input.read(cx).value().to_lowercase();
        let q = q.trim();
        let tab = if q.contains("proble") || q.contains("puzzle") || q.contains("tátic") {
            Some(AppTab::Puzzles)
        } else if q.contains("apren") || q.contains("liç") {
            Some(AppTab::Learn)
        } else if q.contains("trein") || q.contains("motor") {
            Some(AppTab::Train)
        } else if q.contains("assist") || q.contains("video") || q.contains("abertura") {
            Some(AppTab::Watch)
        } else if q.contains("comun") || q.contains("chat") || q.contains("amig") {
            Some(AppTab::Community)
        } else if q.contains("config") || q.contains("outro") {
            Some(AppTab::Other)
        } else if q.contains("jog") || q.contains("partida") || q.contains("xadrez") {
            Some(AppTab::Play)
        } else {
            None
        };
        if let Some(tab) = tab {
            self.tab = tab;
        } else {
            self.tab = AppTab::Play;
            self.notice = "Nenhuma seção encontrada. Busque Jogar, Problemas, Aprender, Treinar, Assistir, Comunidade ou Outros.".into();
        }
        cx.notify();
    }

    fn board(&self, cx: &mut Context<Self>) -> impl IntoElement {
        self.board_for(&self.game, self.selected, BoardInteraction::Play, cx)
    }

    fn board_for(
        &self,
        game: &game::Game,
        selected: Option<Square>,
        interaction: BoardInteraction,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let flipped = interaction == BoardInteraction::Play && (if self.mode == Mode::P2p {
            self.player == Color::Black
        } else {
            self.flip_board
        });
        let targets = selected.map(|sq| game.legal_targets(sq)).unwrap_or_default();
        let mut ranks = Vec::new();
        for visual_rank in 0..8_u32 {
            let mut row = Vec::new();
            for visual_file in 0..8_u32 {
                let file = if flipped { 7 - visual_file } else { visual_file };
                let rank = if flipped { visual_rank } else { 7 - visual_rank };
                let square = Square::new(rank * 8 + file);
                let is_light = (file + rank) % 2 == 1;
                let mut bg = if is_light { 0xEEEED2 } else { 0x769656 };
                if game.last.is_some_and(|(from,to)| from == square || to == square) {
                    bg = if is_light { 0xF6F669 } else { 0xBACA44 };
                }
                if selected == Some(square) {
                    bg = 0xF4D35E;
                }
                let glyph = game.piece_glyph(square);
                let is_white = game.position.board().piece_at(square)
                    .is_some_and(|piece| piece.color == Color::White);
                // Both sides use solid silhouettes. Outline them against both
                // green and cream squares; the original white Unicode chess
                // symbols were hollow and looked translucent.
                let fill = if is_white { 0xFFF8E9 } else { 0x1D211E };
                let outline = if is_white { 0x33372F } else { 0xEFEADC };
                let target = targets.contains(&square);
                let piece_element = if glyph.is_empty() {
                    div().w(px(64.)).h(px(64.))
                        .flex().items_center().justify_center()
                        .text_size(px(20.)).text_color(rgb(0x2F5131))
                        .child(if target { "●" } else { "" })
                } else {
                    let offsets: [(f32, f32); 8] = [
                        (-1.5, 0.), (1.5, 0.), (0., -1.5), (0., 1.5),
                        (-1., -1.), (-1., 1.), (1., -1.), (1., 1.),
                    ];
                    div().relative().w(px(64.)).h(px(64.))
                        .children(offsets.into_iter().map(|(x, y)| {
                            div().absolute().top(px(y)).left(px(x))
                                .w(px(64.)).h(px(64.))
                                .flex().items_center().justify_center()
                                .text_size(px(60.)).text_color(rgb(outline))
                                .font_family("Segoe UI Symbol")
                                .child(glyph)
                        }))
                        .child(
                            div().absolute().inset_0()
                                .flex().items_center().justify_center()
                                .text_size(px(60.)).text_color(rgb(fill))
                                .font_family("Segoe UI Symbol")
                                .child(glyph)
                        )
                };
                row.push(div()
                    .id(format!("square-{square}"))
                    .w(px(72.)).h(px(72.))
                    .flex().items_center().justify_center()
                    .bg(rgb(bg))
                    .cursor_pointer()
                    .hover(|style| style.opacity(0.88))
                    .on_click(cx.listener(move |this, _, _, cx| match interaction {
                        BoardInteraction::Play => this.select_square(square, cx),
                        BoardInteraction::Puzzle => this.select_puzzle_square(square, cx),
                        BoardInteraction::Preview => {}
                    }))
                    .child(piece_element));
            }
            ranks.push(div().flex().children(row));
        }
        div().flex().flex_col().rounded_lg().overflow_hidden().children(ranks)
    }
}

impl Render for ChessApp {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.render_layout(cx)
    }
}

fn main() {
    application().with_assets(assets::Assets).run(|cx| {
        init(cx);
        let bounds = Bounds::centered(None, size(px(1420.), px(900.)), cx);
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            window_min_size: Some(size(px(1220.), px(740.))),
            ..Default::default()
        };
        open_window(options, cx, |window, cx| {
            window.set_window_title("Rookery — Chess in Rust");
            cx.new(|cx| {
                let mut app = ChessApp::new(window, cx);
                app.start_poll(cx);
                app
            })
        }).expect("Não foi possível abrir o tabuleiro");
    });
}
