//! Direct TCP peer-to-peer chess. No central game server is needed.
//! Connections through NAT may require port forwarding or a VPN.
use serde::{Deserialize, Serialize};
use std::{
    io::{BufReader, Read, Write},
    net::{Shutdown, TcpListener, TcpStream},
    sync::{atomic::{AtomicBool, Ordering}, mpsc::{self, Receiver, Sender}, Arc},
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
    stop: Arc<AtomicBool>,
}

impl Peer {
    pub fn host() -> Self {
        Self::start(None, format!("0.0.0.0:{PORT}"))
    }

    pub fn join(address: String) -> Self {
        Self::start(Some(address), String::new())
    }

    fn start(address: Option<String>, bind_address: String) -> Self {
        let (event_tx, incoming) = mpsc::channel();
        let (outgoing, outgoing_rx) = mpsc::channel();
        let stop = Arc::new(AtomicBool::new(false));
        let worker_stop = stop.clone();
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
                    let listener = match TcpListener::bind(&bind_address) {
                        Ok(listener) => listener,
                        Err(e) => {
                            let _ = event_tx.send(Event::Error(format!("Não foi possível hospedar: {e}")));
                            return;
                        }
                    };
                    let bound = listener.local_addr().map(|addr| addr.to_string()).unwrap_or(bind_address);
                    let _ = event_tx.send(Event::Listening(bound));
                    let _ = listener.set_nonblocking(true);
                    loop {
                        if worker_stop.load(Ordering::Relaxed) { return; }
                        match listener.accept() {
                            Ok((stream, _)) => break Ok(stream),
                            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => thread::sleep(Duration::from_millis(75)),
                            Err(e) => break Err(e),
                        }
                    }
                }
            };
            match stream {
                Ok(stream) => {
                    if !worker_stop.load(Ordering::Relaxed) {
                        serve(stream, event_tx, outgoing_rx, worker_stop);
                    }
                },
                Err(e) => { let _ = event_tx.send(Event::Error(format!("Conexão falhou: {e}"))); }
            }
        });
        Self { incoming, outgoing, stop }
    }

    pub fn send(&self, message: Message) -> Result<(), String> {
        self.outgoing.send(message).map_err(|_| "Conexão encerrada".to_string())
    }
}

impl Drop for Peer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

fn serve(stream: TcpStream, tx: Sender<Event>, rx: Receiver<Message>, stop: Arc<AtomicBool>) {
    let _ = stream.set_nodelay(true);
    let _ = stream.set_read_timeout(Some(Duration::from_millis(250)));
    let mut writer = match stream.try_clone() {
        Ok(clone) => clone,
        Err(e) => { let _ = tx.send(Event::Error(e.to_string())); return; }
    };
    let write_tx = tx.clone();
    let writer_stop = stop.clone();
    thread::spawn(move || {
        let send_line = |message: Message, socket: &mut TcpStream| -> std::io::Result<()> {
            serde_json::to_writer(&mut *socket, &message)?;
            socket.write_all(b"\n")?;
            socket.flush()
        };
        if let Err(e) = send_line(Message::Hello { version: PROTOCOL }, &mut writer) {
            let _ = write_tx.send(Event::Error(format!("Erro ao enviar handshake: {e}")));
            return;
        }
        while !writer_stop.load(Ordering::Relaxed) {
            match rx.recv_timeout(Duration::from_millis(250)) {
                Ok(message) => {
                    if let Err(e) = send_line(message, &mut writer) {
                        let _ = write_tx.send(Event::Error(format!("Erro de envio: {e}")));
                        break;
                    }
                }
                Err(mpsc::RecvTimeoutError::Timeout) => continue,
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }
        }
        let _ = writer.shutdown(Shutdown::Both);
    });
    let mut reader = BufReader::new(stream);
    let mut authenticated = false;
    let mut packet = Vec::with_capacity(128);
    let mut byte = [0_u8; 1];
    loop {
        if stop.load(Ordering::Relaxed) { break; }
        match reader.read(&mut byte) {
            Ok(0) => break,
            Ok(_) if byte[0] == b'\n' => {
                let parsed = serde_json::from_slice::<Message>(&packet);
                packet.clear();
                match parsed {
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
                }
            }
            Ok(_) => {
                if packet.len() >= 4096 {
                    let _ = tx.send(Event::Error("Mensagem P2P grande demais".into()));
                    break;
                }
                packet.push(byte[0]);
            }
            Err(e) if matches!(e.kind(), std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock) => continue,
            Err(e) => { let _ = tx.send(Event::Error(format!("Erro de leitura: {e}"))); break; }
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
    #[test]
    fn peers_exchange_moves_over_loopback() {
        let host = Peer::start(None, "127.0.0.1:0".into());
        let listen = host.incoming.recv_timeout(Duration::from_secs(5)).unwrap();
        let Event::Listening(address) = listen else {
            panic!("expected listener address, got {listen:?}");
        };
        let guest = Peer::join(address);
        assert!(matches!(
            host.incoming.recv_timeout(Duration::from_secs(5)).unwrap(),
            Event::Connected
        ));
        assert!(matches!(
            guest.incoming.recv_timeout(Duration::from_secs(5)).unwrap(),
            Event::Connected
        ));
        host.send(Message::Move { ply: 0, uci: "e2e4".into() }).unwrap();
        let event = guest.incoming.recv_timeout(Duration::from_secs(5)).unwrap();
        assert!(matches!(event, Event::Message(Message::Move { ply: 0, uci }) if uci == "e2e4"));
        guest.send(Message::Move { ply: 1, uci: "e7e5".into() }).unwrap();
        let event = host.incoming.recv_timeout(Duration::from_secs(5)).unwrap();
        assert!(matches!(event, Event::Message(Message::Move { ply: 1, uci }) if uci == "e7e5"));
    }

    #[test]
    fn dropping_host_releases_listener() {
        let host = Peer::start(None, "127.0.0.1:0".into());
        let event = host.incoming.recv_timeout(Duration::from_secs(5)).unwrap();
        let Event::Listening(address) = event else { panic!("listener missing"); };
        drop(host);
        // The old listener thread must terminate soon instead of holding a port forever.
        let mut success = false;
        for _ in 0..40 {
            if TcpListener::bind(&address).is_ok() {
                success = true;
                break;
            }
            thread::sleep(Duration::from_millis(50));
        }
        assert!(success, "listener was not released");
    }

}
