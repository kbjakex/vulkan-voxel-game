use std::thread::JoinHandle;

use anyhow::bail;
use flexstr::SharedStr;
use tokio::sync::{mpsc::{UnboundedReceiver, unbounded_channel}, oneshot};

use anyhow::Result;

use crate::{components::NetworkId, net::PlayerChannels};

use self::network_thread::{NetSideChannels, PlayerStateMsg};

pub mod network_thread;
pub mod client_connection;
pub mod login;

#[derive(Debug)]
pub enum LoginResponse {
    Success(Box<[u8]>),
    Denied(&'static [u8])
}

#[derive(Debug)]
pub enum PlayersChanged {
    LoginRequest {
        channel: oneshot::Sender<(NetworkId, LoginResponse)>,
        username: SharedStr,
    },
    Connected {
        username: SharedStr,
        network_id: NetworkId,
        channels: PlayerChannels,
    },
    Disconnect {
        network_id: NetworkId
    }
}

pub struct Channels {
    pub player_join: UnboundedReceiver<PlayersChanged>,
    pub chat_recv: UnboundedReceiver<(NetworkId, SharedStr)>,
    pub player_state_recv: UnboundedReceiver<(NetworkId, u32, PlayerStateMsg)>
}

pub struct NetHandle {
    thread_handle: JoinHandle<()>,
    pub channels: Channels,
}

impl NetHandle {
    pub fn closed(&self) -> bool {
        self.thread_handle.is_finished()
    }

    pub fn poll_joins(&mut self) -> Option<PlayersChanged> {
        self.channels.player_join.try_recv().ok()
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
        channels: Channels {
            player_join: player_join_recv,
            chat_recv,
            player_state_recv
        },
    })
}