mod engine;
mod game;
mod network;

use gpui_kit::component::{
    button::{Button, ButtonVariants},
    input::{Input, InputState},
};
use gpui_kit::*;
use shakmaty::{Color, Position, Role, Square};
use std::{sync::mpsc::{self, Receiver}, time::Duration};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Local,
    Computer,
    P2p,
}

struct ChessApp {
    game: game::Game,
    selected: Option<Square>,
    mode: Mode,
    player: Color,
    connected: bool,
    peer: Option<network::Peer>,
    engine_result: Option<Receiver<Option<String>>>,
    address: Entity<InputState>,
    notice: String,
}

impl ChessApp {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self {
            game: game::Game::default(),
            selected: None,
            mode: Mode::Local,
            player: Color::White,
            connected: false,
            peer: None,
            engine_result: None,
            address: cx.new(|cx| {
                InputState::new(window, cx)
                    .placeholder("IP do anfitrião:50555")
                    .default_value("127.0.0.1:50555")
            }),
            notice: "Escolha o modo de jogo.".into(),
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
                network::Event::Listening(bind) => {
                    self.notice = format!("Aguardando adversário em {bind}. Compartilhe seu IP.");
                }
                network::Event::Connected => {
                    self.connected = true;
                    self.notice = "Conectado! Partida P2P iniciada.".into();
                }
                network::Event::Message(network::Message::Move { ply, uci }) => {
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
                network::Event::Message(network::Message::Resign) => {
                    self.notice = "Adversário desistiu. Você venceu.".into();
                    self.connected = false;
                    self.peer = None;
                }
                network::Event::Message(network::Message::Hello { .. }) => {}
                network::Event::Disconnected => {
                    self.notice = "Adversário desconectou.".into();
                    self.connected = false;
                    self.peer = None;
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
            if self.mode == Mode::Computer && self.game.turn() == Color::Black {
                if let Some(mv) = best {
                    if self.game.make_move(&mv).is_ok() {
                        self.notice = format!("Motor jogou {mv}");
                    }
                }
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
        self.notice = match mode {
            Mode::Local => "Partida local. Brancas começam.".into(),
            Mode::Computer => "Você joga de brancas contra o motor.".into(),
            Mode::P2p => "Configurando conexão P2P...".into(),
        };
        cx.notify();
    }

    fn host(&mut self, cx: &mut Context<Self>) {
        self.reset(Mode::P2p, cx);
        self.peer = Some(network::Peer::host());
        self.notice = format!("Hospedando na porta TCP {}...", network::PORT);
        cx.notify();
    }

    fn join(&mut self, cx: &mut Context<Self>) {
        let address = self.address.read(cx).value().to_string();
        self.reset(Mode::P2p, cx);
        self.player = Color::Black;
        self.peer = Some(network::Peer::join(address));
        self.notice = "Conectando ao anfitrião...".into();
        cx.notify();
    }

    fn schedule_engine(&mut self) {
        let position = self.game.position.clone();
        let (tx, rx) = mpsc::channel();
        self.engine_result = Some(rx);
        std::thread::spawn(move || {
            let result = engine::best_move(&position, 3);
            let _ = tx.send(result);
        });
    }

    fn select_square(&mut self, square: Square, cx: &mut Context<Self>) {
        if self.game.is_finished() || self.engine_result.is_some() {
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
            let uci = format!("{from}{square}{}", if promotion { "q" } else { "" });
            let ply = self.game.moves.len();
            match self.game.make_move(&uci) {
                Ok(()) => {
                    self.selected = None;
                    self.notice.clear();
                    if self.mode == Mode::P2p {
                        if let Some(peer) = &self.peer {
                            if let Err(error) = peer.send(network::Message::Move { ply, uci }) {
                                self.notice = error;
                                self.connected = false;
                            }
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

    fn board(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let flipped = self.mode == Mode::P2p && self.player == Color::Black;
        let targets = self.selected.map(|sq| self.game.legal_targets(sq)).unwrap_or_default();
        let mut ranks = Vec::new();
        for visual_rank in 0..8_u32 {
            let mut row = Vec::new();
            for visual_file in 0..8_u32 {
                let file = if flipped { 7 - visual_file } else { visual_file };
                let rank = if flipped { visual_rank } else { 7 - visual_rank };
                let square = Square::new(rank * 8 + file);
                let is_light = (file + rank) % 2 == 1;
                let mut bg = if is_light { 0xEEEED2 } else { 0x769656 };
                if self.game.last.is_some_and(|(from,to)| from == square || to == square) {
                    bg = if is_light { 0xF6F669 } else { 0xBACA44 };
                }
                if self.selected == Some(square) {
                    bg = 0xF4D35E;
                }
                let glyph = self.game.piece_glyph(square);
                let text_color = if self.game.position.board().piece_at(square)
                    .is_some_and(|p| p.color == Color::White) { 0xF9F9F5 } else { 0x242424 };
                let target = targets.contains(&square);
                row.push(div()
                    .id(format!("square-{square}"))
                    .w(px(64.)).h(px(64.))
                    .flex().items_center().justify_center()
                    .bg(rgb(bg))
                    .cursor_pointer()
                    .hover(|style| style.opacity(0.88))
                    .on_click(cx.listener(move |this, _, _, cx| this.select_square(square, cx)))
                    .child(
                        div().text_size(px(47.)).text_color(rgb(text_color))
                            .font_family("Segoe UI Symbol")
                            .child(if target && glyph.is_empty() { "●" } else { glyph })
                    ));
            }
            ranks.push(div().flex().children(row));
        }
        div().flex().flex_col().rounded_lg().overflow_hidden().children(ranks)
    }
}

impl Render for ChessApp {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let title = self.game.status();
        let mode_name = match self.mode { Mode::Local => "JOGO LOCAL", Mode::Computer => "CONTRA O MOTOR", Mode::P2p => "MULTIPLAYER P2P" };
        let side = div()
            .id("sidebar")
            .w(px(325.)).h(px(655.)).overflow_y_scroll()
            .p_5().rounded_lg().bg(rgb(0x252522))
            .flex().flex_col().gap_4()
            .child(div().text_xs().text_color(rgb(0x82BC5F)).child(mode_name))
            .child(div().text_xl().text_color(rgb(0xF5F5F2)).child(title))
            .child(div().text_sm().text_color(rgb(0xB6B6AE)).child(self.notice.clone()))
            .child(div().h(px(1.)).bg(rgb(0x40403D)))
            .child(div().text_color(rgb(0xEAEAE5)).child("Jogar"))
            .child(Button::new("local").label("Duas pessoas (local)").on_click(cx.listener(|this,_,_,cx| this.reset(Mode::Local, cx))))
            .child(Button::new("computer").primary().label("Jogar contra o motor").on_click(cx.listener(|this,_,_,cx| this.reset(Mode::Computer, cx))))
            .child(div().h(px(1.)).bg(rgb(0x40403D)))
            .child(div().text_color(rgb(0xEAEAE5)).child("Partida P2P direta"))
            .child(Button::new("host").label("Hospedar (brancas)").on_click(cx.listener(|this,_,_,cx| this.host(cx))))
            .child(Input::new(&self.address))
            .child(Button::new("join").label("Entrar pelo IP (pretas)").on_click(cx.listener(|this,_,_,cx| this.join(cx))))
            .child(div().text_xs().text_color(rgb(0xB6B6AE))
                .child("Sem servidor central. Para jogar pela internet, abra a porta TCP 50555 no roteador do anfitrião ou use VPN."))
            .child(div().h(px(1.)).bg(rgb(0x40403D)))
            .child(div().text_sm().text_color(rgb(0xEAEAE5)).child("Histórico de lances"))
            .child(div().id("move-history").max_h(px(180.)).overflow_y_scroll()
                .children(self.game.notation.chunks(2).enumerate().map(|(i, pair)| {
                    let white = pair.first().cloned().unwrap_or_default();
                    let black = pair.get(1).cloned().unwrap_or_default();
                    div().py_1().text_sm().text_color(rgb(0xDCDCD6))
                        .child(format!("{}.  {:<9} {}", i + 1, white, black))
                }).collect::<Vec<_>>()));

        div()
            .size_full().bg(rgb(0x181816))
            .flex().flex_col().p_6().gap_5()
            .child(div().flex().items_center().justify_between()
                .child(div().text_2xl().text_color(rgb(0xF0F0EA)).child("ROOKERY"))
                .child(div().text_sm().text_color(rgb(0xA7A7A0)).child("RUST  ·  GPUI  ·  P2P")))
            .child(div().flex().items_start().justify_center().gap_6()
                .child(div().flex().flex_col().gap_3()
                    .child(div().text_color(rgb(0xEAEAE5)).child(
                        if self.mode == Mode::P2p && self.player == Color::Black { "Você: Pretas" } else { "Brancas" }
                    ))
                    .child(self.board(cx))
                    .child(div().text_color(rgb(0xB7B7B0)).text_sm().child(
                        "Clique na peça e depois na casa de destino. Promoção automática para dama."
                    )))
                .child(side))
    }
}

fn main() {
    application().with_assets(assets::Assets).run(|cx| {
        init(cx);
        let bounds = Bounds::centered(None, size(px(1040.), px(735.)), cx);
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            window_min_size: Some(size(px(990.), px(650.))),
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
