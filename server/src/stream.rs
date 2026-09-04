use std::io;
use std::net::SocketAddr;
use std::thread::JoinHandle;

use futures_util::{SinkExt, StreamExt};
use probe_protocol::{Codec, MAX_SLOTS, Message, PlayerId, Request, WIRE_CAP};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc::{UnboundedSender, unbounded_channel};
use tokio::sync::oneshot;
use tokio_tungstenite::accept_async_with_config;
use tokio_tungstenite::tungstenite::Message as Frame;
use tokio_tungstenite::tungstenite::protocol::WebSocketConfig;

use crate::rooms::{Outbound, Recipient, Room};

const MAX_SOCKETS: usize = 2 * MAX_SLOTS;

pub struct Hosted {
    address: SocketAddr,
    stop: Option<oneshot::Sender<()>>,
    thread: Option<JoinHandle<()>>,
}

struct Held {
    socket: usize,
    player: Option<PlayerId>,
    writer: UnboundedSender<Message>,
}

enum SocketEvent {
    Closed {
        socket: usize,
    },
    Opened {
        socket: usize,
        writer: UnboundedSender<Message>,
    },
    Received {
        socket: usize,
        message: Message,
    },
}

impl Hosted {
    pub fn address(&self) -> SocketAddr {
        self.address
    }

    pub fn serving(address: SocketAddr) -> io::Result<Hosted> {
        let listener = std::net::TcpListener::bind(address)?;
        listener.set_nonblocking(true)?;
        let address = listener.local_addr()?;
        let (stop, stopped) = oneshot::channel();
        let thread = std::thread::Builder::new()
            .name("probe-room".to_string())
            .spawn(move || {
                let Ok(runtime) = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                else {
                    return;
                };
                runtime.block_on(async move {
                    let Ok(listener) = TcpListener::from_std(listener) else {
                        return;
                    };
                    serve(listener, stopped).await;
                });
            })?;
        Ok(Hosted {
            address,
            stop: Some(stop),
            thread: Some(thread),
        })
    }

    pub fn wait(mut self) {
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl Drop for Hosted {
    fn drop(&mut self) {
        drop(self.stop.take());
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn send_to(held: &[Held], from: usize, posts: Vec<Outbound>) {
    for post in posts {
        for one in held.iter().filter(|held| match post.to {
            Recipient::Everyone => true,
            Recipient::EveryoneElse => held.socket != from,
            Recipient::Sender => held.socket == from,
            Recipient::One(player) => held.player == Some(player),
        }) {
            let _ = one.writer.send(post.message.clone());
        }
    }
}

fn route(room: &mut Room, held: &mut [Held], socket: usize, message: Message) -> Vec<Outbound> {
    let Some(at) = held.iter().position(|held| held.socket == socket) else {
        return Vec::new();
    };
    match (held[at].player, message) {
        (Some(player), message) => room.receive(player, message),
        (None, Message::Request(Request::Join { version })) => match room.join(version) {
            Ok(joined) => {
                held[at].player = Some(joined.player);
                joined.posts
            }
            Err(why) => vec![Outbound::to(Recipient::Sender, Message::Notice(why))],
        },
        (None, _) => Vec::new(),
    }
}

async fn serve(listener: TcpListener, mut stopped: oneshot::Receiver<()>) {
    let (events, mut incoming) = unbounded_channel();
    let mut room = Room::opened();
    let mut held: Vec<Held> = Vec::new();
    let mut open = 0usize;
    let mut next = 0usize;
    loop {
        tokio::select! {
            _ = &mut stopped => return,
            accepted = listener.accept() => {
                if let Ok((stream, _)) = accepted
                    && open < MAX_SOCKETS
                {
                    open += 1;
                    next += 1;
                    tokio::spawn(serve_socket(stream, next, events.clone()));
                }
            }
            event = incoming.recv() => match event {
                None => return,
                Some(SocketEvent::Opened { socket, writer }) => held.push(Held {
                    socket,
                    player: None,
                    writer,
                }),
                Some(SocketEvent::Received { socket, message }) => {
                    let posts = route(&mut room, &mut held, socket, message);
                    send_to(&held, socket, posts);
                }
                Some(SocketEvent::Closed { socket }) => {
                    open = open.saturating_sub(1);
                    let Some(at) = held.iter().position(|held| held.socket == socket) else {
                        continue;
                    };
                    let gone = held.remove(at);
                    if let Some(player) = gone.player {
                        let posts = room.leaves(player);
                        send_to(&held, socket, posts);
                    }
                }
            },
        }
    }
}

async fn serve_socket(stream: TcpStream, socket: usize, events: UnboundedSender<SocketEvent>) {
    let config = WebSocketConfig::default()
        .max_message_size(Some(WIRE_CAP))
        .max_frame_size(Some(WIRE_CAP));
    if let Ok(wire) = accept_async_with_config(stream, Some(config)).await {
        let (mut sink, mut source) = wire.split();
        let (writer, mut writes) = unbounded_channel();
        if events.send(SocketEvent::Opened { socket, writer }).is_ok() {
            loop {
                tokio::select! {
                    write = writes.recv() => {
                        let Some(message) = write else { break };
                        if sink.send(Frame::binary(message.encoded())).await.is_err() {
                            break;
                        }
                    }
                    read = source.next() => match read {
                        Some(Ok(Frame::Binary(bytes))) => match Message::decode(&bytes) {
                            Ok(message) => {
                                if events.send(SocketEvent::Received { socket, message }).is_err() {
                                    break;
                                }
                            }

                            Err(_) => continue,
                        },
                        Some(Ok(_)) => continue,
                        Some(Err(_)) | None => break,
                    },
                }
            }
        }
    }

    let _ = events.send(SocketEvent::Closed { socket });
}
