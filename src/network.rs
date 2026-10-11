//! Direct TCP peer-to-peer chess. No central game server is needed.
//! Connections through NAT may require port forwarding or a VPN.
use serde::{Deserialize, Serialize};
use std::{
    io::{BufRead, BufReader, Write},
    net::{TcpListener, TcpStream},
    sync::mpsc::{self, Receiver, Sender},
    thread,
    time::Duration,
};

pub const PORT: u16 = 50555;
const PROTOCOL: u16 = 1;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Message {
    Hello { version: u16 },
    Move { ply: usize, uci: String },
    Resign,
}

#[derive(Debug)]
pub enum Event {
    Listening(String),
    Connected,
    Message(Message),
    Disconnected,
    Error(String),
}

pub struct Peer {
    pub incoming: Receiver<Event>,
    outgoing: Sender<Message>,
}

impl Peer {
    pub fn host() -> Self {
        Self::start(None)
    }

    pub fn join(address: String) -> Self {
        Self::start(Some(address))
    }

    fn start(address: Option<String>) -> Self {
        let (event_tx, incoming) = mpsc::channel();
        let (outgoing, outgoing_rx) = mpsc::channel();
        thread::spawn(move || {
            let stream = match address {
                Some(address) => {
                    let address = match address.parse() {
                        Ok(addr) => addr,
                        Err(_) => {
                            let _ = event_tx.send(Event::Error("Endereço inválido. Use IP:50555".into()));
                            return;
                        }
                    };
                    TcpStream::connect_timeout(&address, Duration::from_secs(8))
                }
                None => {
                    let bind = format("0.0.0.0:{PORT}");
                    let listener = match TcpListener::bind(&bind) {
                        Ok(listener) => listener,
                        Err(e) => {
                            let _ = event_tx.send(Event::Error(format("Não foi possível hospedar: {e}")));
                            return;
                        }
                    };
                    let _ = event_tx.send(Event::Listening(bind));
                    match listener.accept() {
                        Ok((stream, _)) => Ok(stream),
                        Err(e) => Err(e),
                    }
                }
            };
            match stream {
                Ok(stream) => serve(stream, event_tx, outgoing_rx),
                Err(e) => { let _ = event_tx.send(Event::Error(format("Conexão falhou: {e}"))); }
            }
        });
        Self { incoming, outgoing }
    }

    pub fn send(&self, message: Message) -> Result<(), String> {
        self.outgoing.send(message).map_err(|_| "Conexão encerrada".to_string())
    }
}

fn serve(stream: TcpStream, tx: Sender<Event>, rx: Receiver<Message>) {
    let _ = stream.set_nodelay(true);
    let mut writer = match stream.try_clone() {
        Ok(clone) => clone,
        Err(e) => { let _ = tx.send(Event::Error(e.to_string())); return; }
    };
    let write_tx = tx.clone();
    thread::spawn(move || {
        let send_line = |message: Message, socket: &mut TcpStream| -> std::io::Result<()> {
            serde_json::to_writer(&mut *socket, &message)?;
            socket.write_all(b"\n")?;
            socket.flush()
        };
        if let Err(e) = send_line(Message::Hello { version: PROTOCOL }, &mut writer) {
            let _ = write_tx.send(Event::Error(format("Erro ao enviar handshake: {e}")));
            return;
        }
        while let Ok(message) = rx.recv() {
            if let Err(e) = send_line(message, &mut writer) {
                let _ = write_tx.send(Event::Error(format("Erro de envio: {e}")));
                break;
            }
        }
    });
    let mut reader = BufReader::new(stream);
    let mut authenticated = false;
    let mut line = String::new();
    loop {
        line.clear();
        match reader.read_line(&mut line) {
            Ok(0) => break,
            Ok(_) if line.len() > 4096 => {
                let _ = tx.send(Event::Error("Mensagem grande demais".into()));
                break;
            }
            Ok(_) => match serde_json::from_str::<Message>(&line) {
                Ok(Message::Hello { version }) if !authenticated && version == PROTOCOL => {
                    authenticated = true;
                    let _ = tx.send(Event::Connected);
                }
                Ok(Message::Hello { .. }) => {
                    let _ = tx.send(Event::Error("Versão P2P incompatível".into()));
                    break;
                }
                Ok(message) if authenticated => { let _ = tx.send(Event::Message(message)); }
                _ => {
                    let _ = tx.send(Event::Error("Pacote P2P inválido".into()));
                    break;
                }
            },
            Err(e) => { let _ = tx.send(Event::Error(format("Erro de leitura: {e}"))); break; }
        }
    }
    let _ = tx.send(Event::Disconnected);
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn packets_round_trip() {
        let m = Message::Move { ply: 12, uci: "e7e8q".into() };
        let json = serde_json::to_string(&m).unwrap();
        assert_eq!(serde_json::from_str::<Message>(&json).unwrap(), m);
    }
}
