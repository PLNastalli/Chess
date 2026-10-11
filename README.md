# Rookery Chess

Aplicativo desktop de xadrez em **Rust**, com interface **GPUI** (motor de renderização usado pelo Zed) e os componentes de [GPUI Kit](https://gpui-kit.com/). Visual inspirado em plataformas modernas como Chess.com, sem reutilizar marcas ou assets proprietários.

## Recursos do MVP

- Tabuleiro 8×8, peças Unicode, seleção por clique, destaque do último lance e possíveis destinos.
- Regras completas de movimentos via [shakmaty](https://crates.io/crates/shakmaty): roque, en passant, promoção, xeque e xeque-mate.
- Empate por afogamento, material insuficiente, tripla repetição e 50 lances.
- Modo local para dois jogadores na mesma máquina.
- Motor de xadrez próprio em Rust: negamax com poda alfa-beta, avaliação material/posicional e ordenação de capturas, profundidade 3. **Não é Stockfish e não tem classificação Elo aferida.**
- Multiplayer **P2P TCP** entre dois computadores, sem servidor central, com handshake e validação de movimentos.
- Histórico de jogadas com notação SAN.

## Requisitos

Rust estável atualizado e dependências nativas exigidas pelo GPUI Kit no seu sistema.

Consulte [GPUI Kit — Installation](https://gpui-kit.com/docs/installation/).

## Executar

```bash
git clone https://github.com/PLNastalli/Chess
cd Chess
cargo run --release
```

O aplicativo abre um tabuleiro. Escolha **Duas pessoas**, **Jogar contra o motor**, **Hospedar** ou **Entrar pelo IP**.

### P2P na LAN

1. Jogador A abre o aplicativo e clica em **Hospedar (brancas)**.
2. Jogador B abre o aplicativo, escreve `IP_DO_A:50555` no campo de endereço e clica **Entrar pelo IP (pretas)**.
3. O tabuleiro das pretas será invertido automaticamente.

A porta é **TCP 50555**. Para conexões entre redes diferentes, o anfitrião precisa permitir essa porta no firewall e no roteador (redirecionamento de porta), ou ambos precisam estar numa VPN privada que permita conexões diretas. **Não há STUN/TURN, matchmaking, criptografia de ponta a ponta, contas, ranking, anticheat, relógio nem reconexão automática nesta versão.** Evite expor a porta TCP à internet sem compreender esses limites.

### Jogabilidade

Clique em uma peça e na casa de destino. Selecione **dama, torre, bispo ou cavalo** no painel lateral para a próxima promoção. Você pode desfazer lances em partidas locais ou contra o motor, e desistir de partidas P2P.

## Motor independente (UCI)

O mesmo motor de xadrez pode funcionar no terminal como um programa UCI, sem abrir a janela GPUI:

```bash
cargo run --release --bin rookery-engine
```

Exemplo de comandos:

```text
uci
isready
position startpos moves e2e4 e7e5
go depth 3
quit
```

O motor responde com `bestmove <lance>`. Suporta `position startpos`, `position fen`, `go depth` (1 a 6) e `ucinewgame`. Não implementa ainda controle de tempo UCI, ponderação nem busca interrompível.

## Código

```text
src/
  main.rs      interface GPUI e estados de partida
  game.rs      regras, lances legais, histórico e resultados
  engine.rs    motor negamax / alfa-beta
  bin/rookery-engine.rs   executável UCI independente
  network.rs   protocolo P2P TCP, mensagens e conexão
```

## Verificações

```bash
cargo fmt --check
cargo test
cargo clippy --all-targets -- -D warnings
```

A CI de GitHub Actions executa formatação, testes e verificação estática no Windows.

## Licença

MIT. GPUI Kit e shakmaty têm suas próprias licenças. Rookery é um projeto independente, sem associação com Chess.com.
