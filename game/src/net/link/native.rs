//! One socket on the desktop: a runtime on a thread of its own, and two
//! channels to the thread the game runs on.
//!
//! The runtime blocks on that thread and nowhere else: the game's own
//! thread only ever tries a channel, and this whole module is out of the
//! browser build, which is what the rule against blocking is for.

use std::sync::mpsc::{Receiver, TryRecvError, channel};

use futures_util::{SinkExt, StreamExt};
use probe_protocol::WIRE_CAP;
use tokio::sync::mpsc::{UnboundedSender, unbounded_channel};
use tokio_tungstenite::connect_async_with_config;
use tokio_tungstenite::tungstenite::Message as Frame;
use tokio_tungstenite::tungstenite::protocol::WebSocketConfig;

/// One socket to a room. Opening does not wait: bytes said before the
/// handshake finishes are sent when it does, and a socket that never opens
/// reads as closed.
pub(crate) struct Link {
    says: UnboundedSender<Vec<u8>>,
    hears: Receiver<Vec<u8>>,
    closed: bool,
}

impl Link {
    /// Whether the socket has closed, which a room that was never reached
    /// also reads as.
    pub(crate) fn closed(&self) -> bool {
        self.closed
    }

    /// Every message off the socket since the last call, in arrival order.
    pub(crate) fn heard(&mut self) -> Vec<Vec<u8>> {
        let mut heard = Vec::new();
        loop {
            match self.hears.try_recv() {
                Ok(bytes) => heard.push(bytes),
                Err(TryRecvError::Empty) => return heard,
                Err(TryRecvError::Disconnected) => {
                    self.closed = true;
                    return heard;
                }
            }
        }
    }

    /// Opens a socket to the room at `address`, as `host:port`.
    pub(crate) fn opening(address: &str) -> Link {
        let url = super::url(address);
        let (says, writes) = unbounded_channel();
        let (heard, hears) = channel();
        let closed = std::thread::Builder::new()
            .name("probe-socket".to_string())
            .spawn(move || talk(url, writes, heard))
            .is_err();
        Link {
            says,
            hears,
            closed,
        }
    }

    /// Passes `bytes` to the room.
    pub(crate) fn say(&mut self, bytes: Vec<u8>) {
        self.closed |= self.says.send(bytes).is_err();
    }
}

/// The socket's own thread: a runtime, the handshake, and the messages both
/// ways until either end closes it.
fn talk(
    url: String,
    mut writes: tokio::sync::mpsc::UnboundedReceiver<Vec<u8>>,
    heard: std::sync::mpsc::Sender<Vec<u8>>,
) {
    let Ok(runtime) = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    else {
        return;
    };
    runtime.block_on(async move {
        let config = WebSocketConfig::default()
            .max_message_size(Some(WIRE_CAP))
            .max_frame_size(Some(WIRE_CAP));
        let Ok((wire, _)) = connect_async_with_config(url, Some(config), true).await else {
            return;
        };
        let (mut sink, mut source) = wire.split();
        loop {
            tokio::select! {
                write = writes.recv() => {
                    let Some(bytes) = write else { return };
                    if sink.send(Frame::binary(bytes)).await.is_err() {
                        return;
                    }
                }
                read = source.next() => match read {
                    Some(Ok(Frame::Binary(bytes))) => {
                        if heard.send(bytes.to_vec()).is_err() {
                            return;
                        }
                    }
                    // The wire is binary; every other frame is the
                    // protocol's own housekeeping.
                    Some(Ok(_)) => continue,
                    Some(Err(_)) | None => return,
                },
            }
        }
    });
}
