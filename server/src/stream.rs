//! The room on a socket: the accept loop, one task per machine, and the
//! one thread the whole room runs on.

use std::io;
use std::net::SocketAddr;
use std::thread::JoinHandle;

use futures_util::{SinkExt, StreamExt};
use probe_protocol::{MAX_SLOTS, Message, PlayerId, WIRE_CAP, Wire};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc::{UnboundedSender, unbounded_channel};
use tokio::sync::oneshot;
use tokio_tungstenite::accept_async_with_config;
use tokio_tungstenite::tungstenite::Message as Frame;
use tokio_tungstenite::tungstenite::protocol::WebSocketConfig;

use crate::rooms::{Post, Room, To};

/// The most sockets one room holds at once: two per slot, so a machine
/// that reconnects finds room while its old socket is still closing.
const MAX_SOCKETS: usize = 2 * MAX_SLOTS;

/// A room served on this machine: the thread it runs on, and the address it
/// answers at. Dropping it stops the room and closes every socket.
pub struct Hosted {
    address: SocketAddr,
    stop: Option<oneshot::Sender<()>>,
    thread: Option<JoinHandle<()>>,
}

/// One socket the room holds, and the machine on it once it has joined.
struct Held {
    socket: usize,
    player: Option<PlayerId>,
    writer: UnboundedSender<Message>,
}

/// What the room's own task hears.
enum Heard {
    /// A socket closed, however far it got.
    Closed { socket: usize },
    /// A socket finished its handshake, with the channel its writer reads.
    Opened {
        socket: usize,
        writer: UnboundedSender<Message>,
    },
    /// A message off a socket.
    Said { socket: usize, message: Message },
}

impl Hosted {
    /// The address the room answers at, with the port it was given.
    pub fn address(&self) -> SocketAddr {
        self.address
    }

    /// Serves one room at `address`, on a thread of its own.
    ///
    /// Binding happens before this returns, so the address it answers is
    /// the one it holds even when the port asked for was zero.
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

    /// Waits until the room's own thread ends, which is what the standalone
    /// binary does with it.
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
            // The room's thread ends when its stop channel closes; a thread
            // that panicked has nothing left to report.
            let _ = thread.join();
        }
    }
}

/// Sends every post to the sockets it names, `from` being the socket the
/// room is answering.
fn deliver(held: &[Held], from: usize, posts: Vec<Post>) {
    for post in posts {
        for one in held.iter().filter(|held| match post.to {
            To::Everyone => true,
            To::EveryoneElse => held.socket != from,
            To::Sender => held.socket == from,
            To::One(player) => held.player == Some(player),
        }) {
            // A writer whose task has ended is a socket already closing,
            // which the room hears as its own event.
            let _ = one.writer.send(post.message.clone());
        }
    }
}

/// What the room says to `message` off `socket`: a socket that has not
/// joined is only heard asking to.
fn heard(room: &mut Room, held: &mut [Held], socket: usize, message: Message) -> Vec<Post> {
    let Some(at) = held.iter().position(|held| held.socket == socket) else {
        return Vec::new();
    };
    match (held[at].player, message) {
        (Some(player), message) => room.hears(player, message),
        (None, Message::Join { version }) => match room.join(version) {
            Ok(joined) => {
                held[at].player = Some(joined.player);
                joined.posts
            }
            Err(why) => vec![Post::to(To::Sender, Message::Refused(why))],
        },
        // A socket that has not asked to join has nothing else to say.
        (None, _) => Vec::new(),
    }
}

/// Runs one room over `listener` until `stopped` fires.
async fn serve(listener: TcpListener, mut stopped: oneshot::Receiver<()>) {
    let (says, mut hears) = unbounded_channel();
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
                    tokio::spawn(talk(stream, next, says.clone()));
                }
            }
            said = hears.recv() => match said {
                None => return,
                Some(Heard::Opened { socket, writer }) => held.push(Held {
                    socket,
                    player: None,
                    writer,
                }),
                Some(Heard::Said { socket, message }) => {
                    let posts = heard(&mut room, &mut held, socket, message);
                    deliver(&held, socket, posts);
                }
                Some(Heard::Closed { socket }) => {
                    open = open.saturating_sub(1);
                    let Some(at) = held.iter().position(|held| held.socket == socket) else {
                        continue;
                    };
                    let gone = held.remove(at);
                    if let Some(player) = gone.player {
                        let posts = room.leaves(player);
                        deliver(&held, socket, posts);
                    }
                }
            },
        }
    }
}

/// One machine's socket: its handshake, then its messages both ways until
/// either end closes it.
async fn talk(stream: TcpStream, socket: usize, says: UnboundedSender<Heard>) {
    let config = WebSocketConfig::default()
        .max_message_size(Some(WIRE_CAP))
        .max_frame_size(Some(WIRE_CAP));
    if let Ok(wire) = accept_async_with_config(stream, Some(config)).await {
        let (mut sink, mut source) = wire.split();
        let (writer, mut writes) = unbounded_channel();
        if says.send(Heard::Opened { socket, writer }).is_ok() {
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
                                if says.send(Heard::Said { socket, message }).is_err() {
                                    break;
                                }
                            }
                            // Bytes that are not a message are not read.
                            Err(_) => continue,
                        },
                        // The wire is binary; every other frame is the
                        // protocol's own housekeeping.
                        Some(Ok(_)) => continue,
                        Some(Err(_)) | None => break,
                    },
                }
            }
        }
    }
    // The room's task has already ended when this fails, and a socket
    // closing is nothing it needs told.
    let _ = says.send(Heard::Closed { socket });
}
