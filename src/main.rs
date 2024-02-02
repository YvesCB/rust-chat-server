use std::collections::HashMap;
use std::fmt;
use std::io::{Read, Write};
use std::net::{IpAddr, Shutdown, TcpListener, TcpStream};
use std::result;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, SystemTime};

type Result<T> = result::Result<T, ()>;

const SAFE_MODE: bool = true;
const BAN_LIMIT: Duration = Duration::from_secs(10 * 60);

struct Sensitive<T>(T);

impl<T: fmt::Display> fmt::Display for Sensitive<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Self(inner) = self;
        if SAFE_MODE {
            writeln!(f, "[REDACTED]")
        } else {
            inner.fmt(f)
        }
    }
}

enum Message {
    ClientConnected {
        author: Arc<TcpStream>,
    },
    ClientDisconnected {
        author: Arc<TcpStream>,
    },
    NewMessage {
        author: Arc<TcpStream>,
        bytes: Vec<u8>,
    },
}

struct Client {
    conn: Arc<TcpStream>,
    last_message: SystemTime,
    strike_count: i32,
}

fn server(messages: Receiver<Message>) -> Result<()> {
    let mut clients = HashMap::new();
    let mut banned_mfs: HashMap<IpAddr, SystemTime> = HashMap::new();
    loop {
        let msg = messages.recv().expect("The server receiver is not hung up");
        match msg {
            Message::ClientConnected { author } => {
                let author_addr = author
                    .peer_addr()
                    .expect("TODO: cache the peer address of the connection");
                let mut banned_at = banned_mfs.remove(&author_addr.ip());
                let now = SystemTime::now();

                banned_at = banned_at.and_then(|banned_at| {
                    let diff = now
                        .duration_since(banned_at)
                        .expect("TODO: don't crash if clock went backwards");

                    if diff >= BAN_LIMIT {
                        None
                    } else {
                        Some(banned_at)
                    }
                });

                if let Some(banned_at) = banned_at {
                    let diff = now
                        .duration_since(banned_at)
                        .expect("TODO: don't crash if clock went backwards");
                    banned_mfs.insert(author_addr.ip().clone(), banned_at);
                    let mut author = author.as_ref();
                    let _ = writeln!(
                        author,
                        "You are banned MF: {} secs left",
                        (BAN_LIMIT - diff).as_secs_f32()
                    );
                    let _ = author.shutdown(Shutdown::Both);
                } else {
                    clients.insert(
                        author_addr.clone(),
                        Client {
                            conn: author.clone(),
                            last_message: now,
                            strike_count: 0,
                        },
                    );
                }
            }
            Message::ClientDisconnected { author } => {
                let addr = author
                    .peer_addr()
                    .expect("TODO: cache the peer address of the connection");
                clients.remove(&addr);
            }
            Message::NewMessage { author, bytes } => {
                let author_addr = author
                    .peer_addr()
                    .expect("TODO: cache the peer address of the connection");
                for (addr, client) in clients.iter() {
                    if *addr != author_addr {
                        let _ = client.conn.as_ref().write(&bytes);
                    }
                }
            }
        }
    }
}

fn client(stream: Arc<TcpStream>, messages: Sender<Message>) -> Result<()> {
    messages
        .send(Message::ClientConnected {
            author: stream.clone(),
        })
        .map_err(|err| eprintln!("ERROR: could not send message to the server thread: {err}"))?;

    let mut buffer = Vec::new();
    buffer.resize(64, 0);
    loop {
        let n = stream.as_ref().read(&mut buffer).map_err(|err| {
            eprintln!("ERROR: could not read message from client: {err}");
            let _ = messages.send(Message::ClientDisconnected {
                author: stream.clone(),
            });
        })?;
        messages
            .send(Message::NewMessage {
                author: stream.clone(),
                bytes: buffer[0..n].to_vec(),
            })
            .map_err(|err| {
                eprintln!("ERROR: could not read message to the server thread: {err}");
            })?;
    }
}

fn main() -> Result<()> {
    let address = "0.0.0.0:6969";
    let listener = TcpListener::bind(address)
        .map_err(|err| eprintln!("ERROR: could not bind {address}: {}", Sensitive(err)))?;

    println!("INFO: Listening on {}", address);

    let (message_sender, message_receiver) = channel::<Message>();
    thread::spawn(|| server(message_receiver));

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let stream = Arc::new(stream);
                let message_sender = message_sender.clone();
                thread::spawn(|| client(stream, message_sender));
            }
            Err(err) => {
                eprintln!("ERROR: could not accept connection: {err}");
            }
        }
    }

    Ok(())
}
