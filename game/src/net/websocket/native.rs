use std::sync::mpsc::{Receiver, TryRecvError, channel};

use futures_util::{SinkExt, StreamExt};
use probe_protocol::WIRE_CAP;
use tokio::sync::mpsc::{UnboundedSender, unbounded_channel};
use tokio_tungstenite::connect_async_with_config;
use tokio_tungstenite::tungstenite::Message as Frame;
use tokio_tungstenite::tungstenite::protocol::WebSocketConfig;

pub(crate) struct WebSocket {
    tx: UnboundedSender<Vec<u8>>,
    rx: Receiver<Vec<u8>>,
    closed: bool,
}

impl WebSocket {
    pub(crate) fn closed(&self) -> bool {
        self.closed
    }

    pub(crate) fn received(&mut self) -> Vec<Vec<u8>> {
        let mut received = Vec::new();
        loop {
            match self.rx.try_recv() {
                Ok(bytes) => received.push(bytes),
                Err(TryRecvError::Empty) => return received,
                Err(TryRecvError::Disconnected) => {
                    self.closed = true;
                    return received;
                }
            }
        }
    }

    pub(crate) fn opening(address: &str) -> WebSocket {
        let url = super::url(address);
        let (tx, tx_source) = unbounded_channel();
        let (rx_source, rx) = channel();
        let closed = std::thread::Builder::new()
            .name("probe-socket".to_string())
            .spawn(move || handler(url, tx_source, rx_source))
            .is_err();
        WebSocket { tx, rx, closed }
    }

    pub(crate) fn send(&mut self, bytes: Vec<u8>) {
        self.closed |= self.tx.send(bytes).is_err();
    }
}

fn handler(
    url: String,
    mut tx_source: tokio::sync::mpsc::UnboundedReceiver<Vec<u8>>,
    rx_source: std::sync::mpsc::Sender<Vec<u8>>,
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
                write = tx_source.recv() => {
                    let Some(bytes) = write else { return };
                    if sink.send(Frame::binary(bytes)).await.is_err() {
                        return;
                    }
                }
                read = source.next() => match read {
                    Some(Ok(Frame::Binary(bytes))) => {
                        if rx_source.send(bytes.to_vec()).is_err() {
                            return;
                        }
                    }

                    Some(Ok(_)) => continue,
                    Some(Err(_)) | None => return,
                },
            }
        }
    });
}
