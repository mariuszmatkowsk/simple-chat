use std::net::{TcpListener, TcpStream, SocketAddr};
use std::io::{Write, Read};
use std::result;
use std::sync::mpsc::{channel, Sender, Receiver};
use std::sync::Arc;
use std::collections::HashMap;

type Result<T> = result::Result<T, ()>;

enum Message {
    ClientConnected { address: SocketAddr, stream: Arc<TcpStream> },
    ClientDisconnected { address: SocketAddr },
    NewMessage { author: SocketAddr },
}

fn server(receiver: Receiver<Message>) -> Result<()> {
    let clients: HashMap<SocketAddr, Arc<TcpStream>> = HashMap::new();
    loop {
        let message = receiver.recv().map_err(|err| {
            eprintln!("ERROR: connection hang up: {err}");
        })?;

        match message {
            Message::ClientConnected { address, stream }=> todo!(),
            Message::ClientDisconnected { address } => todo!(),
            Message::NewMessage { author }=> todo!(),
        }
    }
}

fn client(stream: Arc<TcpStream>, sender: Sender<Message>) -> Result<()> {
    let address = stream.peer_addr().map_err(|err| {
        eprintln!("ERROR: could not get peer address: {err}");
    })?;

    stream.as_ref().write(b"You are connected to awesome server").map_err(|err| {
        eprintln!("ERROR: could not send message over tcp to {address}: {err}");
    })?;

    sender.send(Message::ClientConnected { address, stream: stream.clone() }).map_err(|err| {
        eprintln!("ERROR: could not send message to server: {err}");
    })?;
   
    let mut buff = [0; 64];
    loop {
        let n = stream.as_ref().read(&mut buff).map_err(|err| {
            eprintln!("ERROR: could not read data from {address}: {err}");
        })?;

        if n == 0 {
            // Todo: Connection closed by client
        } else {
            // Todo: send message to server
        }

        
    }

}

fn main() -> Result<()> {
    let address = "0.0.0.0:6969";
    let listener = TcpListener::bind(address).map_err(|err| {
        eprintln!("ERROR: could not bound TcpListener to  address {address}: {err}");
    })?;

    let (sender, receiver) = channel();

    std::thread::spawn(|| server(receiver));

    for stream in listener.incoming() {
        match stream {
            Ok(mut stream) => {
                let sender = sender.clone();
                let stream = Arc::new(stream);
                std::thread::spawn(move || client(stream, sender));
            },
            Err(e) => {
                eprintln!("ERROR: could not accept connection: {e}");

            }
        }
    }

    Ok(())
}
