use std::fmt;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::result;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread;

type Result<T> = result::Result<T, ()>;

const SAFE_MODE: bool = true;

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
    ClientConnected,
    ClientDisconnected,
    NewMessage(Vec<u8>),
}

fn server(messages: Receiver<Message>) -> Result<()> {
    todo!()
}

fn client(mut stream: TcpStream, messages: Sender<Message>) -> Result<()> {
    messages
        .send(Message::ClientConnected)
        .map_err(|err| eprintln!("ERROR: could not send message to the server thread: {err}"))?;

    let mut buffer = Vec::new();
    buffer.resize(64, 0);
    loop {
        let n = stream.read(&mut buffer).map_err(|err| {
            messages.send(Message::ClientDisconnected);
        })?;
        messages.send(Message::NewMessage(buffer[0..n].to_vec()));
    }
}

fn main() -> Result<()> {
    let address = "0.0.0.0:6969";
    let listener = TcpListener::bind(address)
        .map_err(|err| eprintln!("ERROR: could not bind {address}: {}", Sensitive(err)))?;

    println!("INFO: Listening on {}", Sensitive(address));

    let (message_sender, message_receiver) = channel::<Message>();
    thread::spawn(|| server(message_receiver));

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
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
