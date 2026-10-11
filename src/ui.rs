//! Navigation, home-like layout and independent activity pages.
use crate::{AppTab, BoardInteraction, ChessApp, Mode};
use gpui_kit::component::{
    button::{Button, ButtonVariants},
    input::Input,
};
use gpui_kit::*;
use shakmaty::Role;

const BACKGROUND: u32 = 0x302E2B;
const SIDEBAR: u32 = 0x242321;
const PANEL: u32 = 0x262522;
const TEXT: u32 = 0xF4F3F0;
const MUTED: u32 = 0xBEBAB3;
const GREEN: u32 = 0x81B64C;

impl ChessApp {
    fn change_tab(&mut self, tab: AppTab, cx: &mut Context<Self>) {
        self.tab = tab;
        cx.notify();
    }

    fn navigation_item(
        &self,
        id: &'static str,
        glyph: &'static str,
        title: &'static str,
        tab: AppTab,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let active = self.tab == tab;
        div()
            .id(id)
            .w_full()
            .h(px(46.))
            .px_3()
            .flex()
            .items_center()
            .gap_3()
            .rounded_md()
            .bg(rgb(if active { 0x383633 } else { SIDEBAR }))
            .cursor_pointer()
            .hover(|style| style.bg(rgb(0x373532)))
            .on_click(cx.listener(move |this, _, _, cx| this.change_tab(tab, cx)))
            .child(
                div().w(px(25.)).text_xl().text_color(rgb(if active { GREEN } else { 0xD5D2CB }))
                    .child(glyph),
            )
            .child(div().text_sm().text_color(rgb(TEXT)).child(title))
    }

    fn sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .w(px(200.))
            .flex_shrink_0()
            .h_full()
            .bg(rgb(SIDEBAR))
            .p_3()
            .flex()
            .flex_col()
            .gap_2()
            .child(
                div().h(px(62.)).px_3().rounded_lg().bg(rgb(0x32302D))
                    .flex().items_center().gap_2()
                    .child(div().text_3xl().text_color(rgb(GREEN)).child("♜"))
                    .child(div().text_xl().text_color(rgb(TEXT)).child("Rookery")),
            )
            .child(self.navigation_item("nav-play", "♟", "Jogar", AppTab::Play, cx))
            .child(self.navigation_item("nav-puzzles", "◆", "Problemas", AppTab::Puzzles, cx))
            .child(self.navigation_item("nav-learn", "▤", "Aprender", AppTab::Learn, cx))
            .child(self.navigation_item("nav-train", "◎", "Treinar", AppTab::Train, cx))
            .child(self.navigation_item("nav-watch", "▶", "Assistir", AppTab::Watch, cx))
            .child(self.navigation_item("nav-community", "♙", "Comunidade", AppTab::Community, cx))
            .child(self.navigation_item("nav-other", "⚙", "Outros", AppTab::Other, cx))
            .child(div().flex_1())
            .child(div().text_xs().text_color(rgb(MUTED)).child("PESQUISAR RECURSOS"))
            .child(Input::new(&self.search_input))
            .child(
                Button::new("search-tab").label("Buscar")
                    .on_click(cx.listener(|this, _, _, cx| this.search_sections(cx))),
            )
            .child(div().h(px(1.)).bg(rgb(0x43413B)))
            .child(div().text_sm().text_color(rgb(GREEN)).child("Sem cadastro obrigatório"))
            .child(div().text_xs().text_color(rgb(MUTED)).child("Rust • GPUI Kit • P2P"))
            .child(div().text_xs().text_color(rgb(MUTED)).child("Português (BR)"))
    }

    fn section_header(&self, label: &'static str, caption: &'static str) -> impl IntoElement {
        div().flex().flex_col().gap_2()
            .child(div().text_3xl().text_color(rgb(TEXT)).child(label))
            .child(div().text_sm().text_color(rgb(MUTED)).child(caption))
    }

    fn game_controls(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let turn = self.game_over.clone().unwrap_or_else(|| self.game.status());
        div().w(px(365.)).flex_shrink_0()
            .p_5().rounded_lg().bg(rgb(PANEL)).flex().flex_col().gap_3()
            .child(div().text_xs().text_color(rgb(GREEN)).child("ROOKERY • XADREZ"))
            .child(div().text_2xl().text_color(rgb(TEXT)).child(turn))
            .child(div().text_sm().text_color(rgb(MUTED)).child(self.notice.clone()))
            .child(div().h(px(1.)).bg(rgb(0x44413C)))
            .child(div().text_lg().text_color(rgb(TEXT)).child("Escolha seu jogo"))
            .child(Button::new("vs-engine").primary().label("Jogar contra o motor")
                .on_click(cx.listener(|this, _, _, cx| {
                    this.reset(Mode::Computer, cx);
                    this.tab = AppTab::Play;
                    cx.notify();
                })))
            .child(Button::new("local-two").label("Jogar no mesmo computador")
                .on_click(cx.listener(|this, _, _, cx| this.reset(Mode::Local, cx))))
            .child(Button::new("undo-move").label("Desfazer lance")
                .on_click(cx.listener(|this, _, _, cx| this.undo(cx))))
            .child(div().h(px(1.)).bg(rgb(0x44413C)))
            .child(div().text_lg().text_color(rgb(TEXT)).child("Multiplayer P2P"))
            .child(Button::new("p2p-host").primary().label("Hospedar partida")
                .on_click(cx.listener(|this, _, _, cx| this.host(cx))))
            .child(Input::new(&self.address))
            .child(Button::new("p2p-join").label("Entrar usando IP")
                .on_click(cx.listener(|this, _, _, cx| this.join(cx))))
            .child(Button::new("p2p-resign").label("Desistir da partida")
                .on_click(cx.listener(|this, _, _, cx| this.resign(cx))))
            .child(div().text_xs().text_color(rgb(MUTED))
                .child("O anfitrião precisa abrir TCP 50555. Em outro computador, não use 127.0.0.1."))
            .child(div().h(px(1.)).bg(rgb(0x44413C)))
            .child(div().text_sm().text_color(rgb(TEXT)).child("Promoção de peão"))
            .child(div().flex().flex_col().gap_2()
                .child(div().flex().gap_2()
                    .child(Button::new("queen-promotion").label("Dama")
                        .on_click(cx.listener(|this, _, _, cx| {this.promotion = Role::Queen; cx.notify();})))
                    .child(Button::new("rook-promotion").label("Torre")
                        .on_click(cx.listener(|this, _, _, cx| {this.promotion = Role::Rook; cx.notify();}))))
                .child(div().flex().gap_2()
                    .child(Button::new("bishop-promotion").label("Bispo")
                        .on_click(cx.listener(|this, _, _, cx| {this.promotion = Role::Bishop; cx.notify();})))
                    .child(Button::new("knight-promotion").label("Cavalo")
                        .on_click(cx.listener(|this, _, _, cx| {this.promotion = Role::Knight; cx.notify();}))))
                .child(div().text_xs().text_color(rgb(GREEN)).child(match self.promotion {
                    Role::Queen => "Selecionado: Dama",
                    Role::Rook => "Selecionado: Torre",
                    Role::Bishop => "Selecionado: Bispo",
                    Role::Knight => "Selecionado: Cavalo",
                    _ => "Selecionado: Dama",
                })))
    }

    fn moves_panel(&self) -> impl IntoElement {
        div().w_full().p_4().rounded_lg().bg(rgb(PANEL))
            .flex().flex_col().gap_2()
            .child(div().text_sm().text_color(rgb(TEXT)).child("Lances da partida"))
            .child(div().id("playing-moves").max_h(px(130.)).overflow_y_scroll().flex().flex_col()
                .children(self.game.notation.chunks(2).enumerate().map(|(i, pair)| {
                    let white = pair.first().cloned().unwrap_or_default();
                    let black = pair.get(1).cloned().unwrap_or_default();
                    div().py_1().text_sm().text_color(rgb(MUTED))
                        .child(format!("{}.  {:<10} {}", i + 1, white, black))
                }).collect::<Vec<_>>()))
    }

    fn chat_panel(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let messages = self.chat_messages.iter().rev().map(|msg| {
            div().w_full().rounded_md().p_2().bg(rgb(if msg.mine { 0x394D2E } else { 0x393733 }))
                .flex().flex_col().gap_1()
                .child(div().text_xs().text_color(rgb(GREEN))
                    .child(if msg.mine { "Você" } else { "Adversário" }))
                .child(div().text_sm().text_color(rgb(TEXT)).child(msg.text.clone()))
        }).collect::<Vec<_>>();
        div().w_full().p_4().rounded_lg().bg(rgb(PANEL)).flex().flex_col().gap_3()
            .child(div().text_lg().text_color(rgb(TEXT)).child("Chat P2P"))
            .child(div().text_xs().text_color(rgb(MUTED)).child(
                if self.connected { "Conectado • mensagens diretas" } else { "Conecte para conversar" }
            ))
            .child(div().id("chat-history-new").h(px(145.)).overflow_y_scroll()
                .flex().flex_col().gap_2().children(messages))
            .child(Input::new(&self.chat_input))
            .child(Button::new("send-p2p-chat").primary().label("Enviar mensagem")
                .on_click(cx.listener(|this, _, window, cx| this.send_chat(window, cx))))
            .child(div().text_xs().text_color(rgb(MUTED))
                .child("Sem criptografia: não envie informações sensíveis."))
    }

    fn play_page(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div().w_full().h_full().id("play-content").overflow_y_scroll()
            .flex().flex_col().gap_5().p_5()
            .child(div().flex().gap_3().items_center()
                .child(div().text_2xl().text_color(rgb(TEXT)).child("Jogue xadrez"))
                .child(div().text_sm().text_color(rgb(GREEN)).child("Online P2P ou offline")))
            .child(div().flex().items_start().gap_6()
                .child(div().flex().flex_col().gap_3().flex_shrink_0()
                    .child(div().text_sm().text_color(rgb(TEXT)).child(
                        if self.mode == Mode::P2p && self.player == shakmaty::Color::Black {
                            "Adversário • Brancas"
                        } else {
                            "Pretas"
                        }))
                    .child(self.board(cx))
                    .child(div().text_sm().text_color(rgb(TEXT)).child(
                        if self.mode == Mode::P2p && self.player == shakmaty::Color::Black {
                            "Você • Pretas"
                        } else {
                            "Brancas"
                        }))
                    .child(div().text_xs().text_color(rgb(MUTED))
                        .child("Clique na peça e na casa de destino."))
                    .child(self.moves_panel()))
                .child(div().flex().flex_col().gap_4()
                    .child(self.game_controls(cx))
                    .child(self.chat_panel(cx))))
    }

    fn puzzle_page(&self, cx: &mut Context<Self>) -> impl IntoElement {
        const PUZZLE_NAMES: [&str; 2] = ["Final de dama", "Rede de mate"];
        div().w_full().h_full().id("puzzle-content").overflow_y_scroll()
            .p_6().flex().flex_col().gap_5()
            .child(self.section_header("Problemas de xadrez",
                "Encontre o mate em um lance. São posições reais, validadas pelo motor."))
            .child(div().flex().items_start().gap_6()
                .child(self.board_for(&self.puzzle_game, self.puzzle_selected, BoardInteraction::Puzzle, cx))
                .child(div().w(px(360.)).p_5().rounded_lg().bg(rgb(PANEL))
                    .flex().flex_col().gap_4()
                    .child(div().text_xl().text_color(rgb(TEXT))
                        .child(PUZZLE_NAMES[self.puzzle_index]))
                    .child(div().text_lg().text_color(rgb(GREEN))
                        .child(if self.puzzle_solved { "Correto! Problema resolvido." }
                               else { "Brancas jogam. Mate em 1." }))
                    .child(div().text_sm().text_color(rgb(MUTED))
                        .child(self.puzzle_notice.clone()))
                    .child(Button::new("puzzle-next").primary().label("Próximo problema")
                        .on_click(cx.listener(|this, _, _, cx| this.next_puzzle(cx))))
                    .child(Button::new("puzzle-retry").label("Reiniciar problema")
                        .on_click(cx.listener(|this, _, _, cx| this.restart_puzzle(cx))))
                    .child(div().text_sm().text_color(rgb(MUTED))
                        .child(format!("Problema {} de 2", self.puzzle_index + 1)))))
    }

    fn lesson_page(&self, cx: &mut Context<Self>) -> impl IntoElement {
        const LESSONS: [(&str, &str, &str); 4] = [
            ("1. Controle o centro", "Peões em e4 e d4 disputam espaço e abrem linhas para as peças.",
             "Exemplo: 1. e4 e5"),
            ("2. Desenvolva as peças", "Tire cavalos e bispos das casas iniciais antes de mover a dama repetidamente.",
             "Exemplo: 1. e4 e5 2. Cf3"),
            ("3. Proteja o rei", "Faça o roque quando for seguro e evite abrir o centro com o rei desprotegido.",
             "Exemplo: 1. e4 e5 2. Cf3 Cc6 3. Bb5 a6 4. Ba4 Cf6 5. O-O"),
            ("4. Procure táticas", "Cheque, captura e ameaça: considere essas ideias a cada lance.",
             "Exercício: encontre mates em um lance na aba Problemas."),
        ];
        let (name, description, example) = LESSONS[self.lesson_index];
        div().w_full().h_full().id("lesson-content").overflow_y_scroll()
            .p_6().flex().flex_col().gap_5()
            .child(self.section_header("Aprender xadrez",
                "Lições iniciais, práticas e independentes de conta online."))
            .child(div().w(px(600.)).p_6().rounded_lg().bg(rgb(PANEL))
                .flex().flex_col().gap_5()
                .child(div().text_xs().text_color(rgb(GREEN))
                    .child(format!("LIÇÃO {} / {}", self.lesson_index + 1, LESSONS.len())))
                .child(div().text_3xl().text_color(rgb(TEXT)).child(name))
                .child(div().text_lg().text_color(rgb(TEXT)).child(description))
                .child(div().p_4().bg(rgb(0x34322F)).rounded_md()
                    .text_color(rgb(MUTED)).child(example))
                .child(Button::new("next-lesson").primary().label("Próxima lição")
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.lesson_index = (this.lesson_index + 1) % 4;
                        cx.notify();
                    })))
                .child(Button::new("practice-lesson").label("Praticar contra o motor")
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.reset(Mode::Computer, cx);
                        this.tab = AppTab::Play;
                        cx.notify();
                    }))))
    }

    fn train_page(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div().w_full().h_full().id("train-content").overflow_y_scroll()
            .p_6().flex().flex_col().gap_5()
            .child(self.section_header("Treinar",
                "Ajuste a profundidade do motor e jogue uma partida de treinamento."))
            .child(div().w(px(600.)).p_6().rounded_lg().bg(rgb(PANEL))
                .flex().flex_col().gap_4()
                .child(div().text_lg().text_color(rgb(TEXT)).child("Dificuldade do motor"))
                .child(div().text_2xl().text_color(rgb(GREEN))
                    .child(format!("Profundidade {} (negamax / alfa-beta)", self.engine_depth)))
                .child(div().text_sm().text_color(rgb(MUTED))
                    .child("Valores maiores calculam mais lances e podem demorar mais."))
                .child(div().flex().gap_3()
                    .child(Button::new("depth-2").label("Iniciante (2)")
                        .on_click(cx.listener(|this, _, _, cx| { this.engine_depth = 2; cx.notify(); })))
                    .child(Button::new("depth-3").label("Normal (3)")
                        .on_click(cx.listener(|this, _, _, cx| { this.engine_depth = 3; cx.notify(); })))
                    .child(Button::new("depth-4").label("Desafio (4)")
                        .on_click(cx.listener(|this, _, _, cx| { this.engine_depth = 4; cx.notify(); }))))
                .child(Button::new("start-training").primary().label("Jogar contra o motor")
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.reset(Mode::Computer, cx);
                        this.tab = AppTab::Play;
                        cx.notify();
                    })))
                .child(Button::new("train-tactics").label("Treinar mates em 1")
                    .on_click(cx.listener(|this, _, _, cx| this.change_tab(AppTab::Puzzles, cx)))))
    }

    fn watch_page(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div().w_full().h_full().id("watch-content").overflow_y_scroll()
            .p_6().flex().flex_col().gap_5()
            .child(self.section_header("Assistir e estudar",
                "Reproduza uma abertura clássica, lance a lance, no tabuleiro."))
            .child(div().flex().gap_6().items_start()
                .child(self.board_for(&self.study_game, None, BoardInteraction::Preview, cx))
                .child(div().w(px(360.)).p_5().rounded_lg().bg(rgb(PANEL))
                    .flex().flex_col().gap_4()
                    .child(div().text_xl().text_color(rgb(TEXT)).child("Abertura Espanhola"))
                    .child(div().text_sm().text_color(rgb(MUTED))
                        .child("Sequência demonstrativa de lances de abertura, não uma transmissão ao vivo."))
                    .child(div().text_2xl().text_color(rgb(GREEN))
                        .child(format!("Lance {} / 16", self.study_ply)))
                    .child(div().flex().gap_2()
                        .child(Button::new("watch-back").label("Anterior")
                            .on_click(cx.listener(|this, _, _, cx| this.study_step(false, cx))))
                        .child(Button::new("watch-forward").primary().label("Próximo")
                            .on_click(cx.listener(|this, _, _, cx| this.study_step(true, cx)))))
                    .child(Button::new("watch-restart").label("Voltar ao início")
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.study_ply = 0;
                            this.study_game = crate::game::Game::default();
                            cx.notify();
                        })))
                    .child(div().id("study-notation").max_h(px(190.)).overflow_y_scroll()
                        .flex().flex_col().gap_1()
                        .children(self.study_game.notation.chunks(2).enumerate().map(|(i, pair)| {
                            div().text_sm().text_color(rgb(MUTED))
                                .child(format!("{}. {} {}", i+1, pair[0],
                                    pair.get(1).map(String::as_str).unwrap_or("")))
                        }).collect::<Vec<_>>()))))
    }

    fn community_page(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div().w_full().h_full().id("community-content").overflow_y_scroll()
            .p_6().flex().flex_col().gap_5()
            .child(self.section_header("Comunidade P2P",
                "Jogue e converse com alguém sem conta e sem servidor central."))
            .child(div().flex().gap_5().items_start()
                .child(div().w(px(390.)).p_5().rounded_lg().bg(rgb(PANEL))
                    .flex().flex_col().gap_4()
                    .child(div().text_lg().text_color(rgb(TEXT)).child("Partida entre amigos"))
                    .child(div().text_sm().text_color(rgb(MUTED)).child(self.notice.clone()))
                    .child(Button::new("community-host").primary().label("Hospedar partida")
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.host(cx);
                            this.tab = AppTab::Community;
                            cx.notify();
                        })))
                    .child(Input::new(&self.address))
                    .child(Button::new("community-join").label("Entrar com IP:50555")
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.join(cx);
                            this.tab = AppTab::Community;
                            cx.notify();
                        })))
                    .child(div().text_sm().text_color(rgb(MUTED))
                        .child("O anfitrião compartilha seu IPv4. Na mesma rede, use o IP local; na internet, VPN ou redirecionamento TCP 50555."))
                    .child(Button::new("community-play").label("Abrir tabuleiro")
                        .on_click(cx.listener(|this, _, _, cx| this.change_tab(AppTab::Play, cx))))
                )
                .child(div().w(px(390.)).child(self.chat_panel(cx))))
    }

    fn other_page(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div().w_full().h_full().id("other-content").overflow_y_scroll()
            .p_6().flex().flex_col().gap_5()
            .child(self.section_header("Outros recursos",
                "Preferências do aplicativo, informações e comandos úteis."))
            .child(div().w(px(600.)).p_6().rounded_lg().bg(rgb(PANEL))
                .flex().flex_col().gap_4()
                .child(div().text_xl().text_color(rgb(TEXT)).child("Tabuleiro"))
                .child(div().text_sm().text_color(rgb(MUTED))
                    .child("A orientação no modo P2P segue automaticamente a sua cor."))
                .child(Button::new("flip-board").primary().label("Inverter tabuleiro local")
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.flip_board = !this.flip_board;
                        cx.notify();
                    })))
                .child(div().text_sm().text_color(rgb(GREEN))
                    .child(if self.flip_board { "Orientação local invertida" }
                        else { "Orientação local padrão" }))
                .child(div().h(px(1.)).bg(rgb(0x44413C)))
                .child(div().text_lg().text_color(rgb(TEXT)).child("Sobre o Rookery"))
                .child(div().text_sm().text_color(rgb(MUTED))
                    .child("Aplicativo independente de xadrez em Rust, GPUI Kit e shakmaty. Motor negamax UCI, partidas locais e multiplayer TCP P2P."))
                .child(div().text_sm().text_color(rgb(MUTED))
                    .child("Sem contas, matchmaking, streaming, criptografia ou ranking online nesta versão."))
                .child(Button::new("go-to-play").label("Voltar para Jogar")
                    .on_click(cx.listener(|this, _, _, cx| this.change_tab(AppTab::Play, cx)))))
    }

    pub(crate) fn render_layout(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let content = match self.tab {
            AppTab::Play => self.play_page(cx).into_any_element(),
            AppTab::Puzzles => self.puzzle_page(cx).into_any_element(),
            AppTab::Learn => self.lesson_page(cx).into_any_element(),
            AppTab::Train => self.train_page(cx).into_any_element(),
            AppTab::Watch => self.watch_page(cx).into_any_element(),
            AppTab::Community => self.community_page(cx).into_any_element(),
            AppTab::Other => self.other_page(cx).into_any_element(),
        };
        div().size_full().flex().bg(rgb(BACKGROUND))
            .child(self.sidebar(cx))
            .child(div().id("workspace").flex_1().h_full().overflow_hidden().child(content))
    }
}
