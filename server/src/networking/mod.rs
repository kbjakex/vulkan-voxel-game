use std::thread::JoinHandle;

use anyhow::bail;
use flexstr::SharedStr;
use tokio::sync::{mpsc::{UnboundedReceiver, unbounded_channel, error::TryRecvError, self}, oneshot};

use anyhow::Result;

use crate::{components::net::PlayerConnection};

use self::network_thread::{NetSideChannels, PlayerStateMsg};

pub mod network_thread;
pub mod client_connection;
pub mod login;
pub mod channels;

#[derive(Debug)]
pub enum PlayersChanged {
    NetworkIdRequest {
        channel: mpsc::Sender<shared::protocol::NetworkId>,
    },
    Connected {
        username: SharedStr,
        network_id: shared::protocol::NetworkId,
        channels: PlayerConnection,
    },
    Disconnect {
        network_id: shared::protocol::NetworkId
    }
}

pub struct Channels {
    pub player_join: UnboundedReceiver<PlayersChanged>,
    pub chat_recv: UnboundedReceiver<(shared::protocol::NetworkId, SharedStr)>,
    pub player_state_recv: UnboundedReceiver<(shared::protocol::NetworkId, PlayerStateMsg)>
}

pub struct NetHandle {
    thread_handle: JoinHandle<()>,
    closed: bool,
    pub channels: Channels,
}

impl NetHandle {
    pub fn closed(&self) -> bool {
        self.closed
    }

    pub fn poll_joins(&mut self) -> Option<PlayersChanged> {
        match self.channels.player_join.try_recv() {
            Ok(val) => Some(val),
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => {
                self.closed = true;
                None
            }
        }
    }
}

pub fn init() -> Result<NetHandle> {
    let (player_join_send, player_join_recv) = unbounded_channel();
    let (chat_send, chat_recv) = unbounded_channel();
    let (player_state_send, player_state_recv) = unbounded_channel();


    let channels = NetSideChannels {
        chat_send,
        player_join_send,
        player_state_send
    };

    let (tx, rx) = oneshot::channel();
    let thread_handle = std::thread::spawn(move || {
        network_thread::start(tx, channels);
    });

    // Don't start loading the server until networking is confirmed to be working
    match rx.blocking_recv() {
        Ok(true) => {}
        Ok(false) => bail!("Failed to start the networking thread!"),
        Err(e) => bail!("Error while waiting for network thread to start: {}", e),
    }

    Ok(NetHandle {
        thread_handle,
        closed: false,
        channels: Channels {
            player_join: player_join_recv,
            chat_recv,
            player_state_recv
        },
    })
}