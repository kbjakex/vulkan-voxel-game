use std::{net::SocketAddr, thread::JoinHandle, time::Instant};

use flexstr::SharedStr;
use glam::{Vec3, Vec2};
use hecs::Entity;
use shared::protocol::NetworkId;
use tokio::sync::{
    mpsc::{unbounded_channel, UnboundedSender},
    oneshot,
};

use crate::states::game::input_recorder::InputSnapshot;

use self::network_thread::NetSideChannels;

pub mod connection;
mod network_thread;

pub struct LoginResponse {
    pub nid: NetworkId,
    pub position: Vec3,
    pub head_rotation: Vec2,
    pub world_seed: u64,
}


#[derive(Clone, Copy)]
pub enum EntityStateMsg {
    EntityAdded {
        id: NetworkId,
        position: Vec3,
        head_rotation: Vec2
    },
    EntityRemoved {
        id: NetworkId,
    },
    EntityMoved {
        id: NetworkId,
        delta_pos: Vec3,
        delta_head_rotation: Vec2,
    },
    InputValidated {
        tag: u16,
        packets_lost: u8,
        server_pos: Vec3,
        server_head_rot: Vec2,
    }
}

pub enum S2C {
    Chat(SharedStr),
    EntityState(Box<[EntityStateMsg]>),
    Statistics{ ping: u32, }
}

#[derive(Copy, Clone)]
pub enum DisconnectReason {
    Unknown
}

pub struct Channels {
    pub incoming: tokio::sync::mpsc::Receiver<S2C>,

    pub chat: UnboundedSender<SharedStr>,
    pub player_state: UnboundedSender<Box<[InputSnapshot]>>,

    pub on_disconnect: oneshot::Receiver<DisconnectReason>,
    pub stop_network_thread: Option<oneshot::Sender<()>>,
}

struct NetThreadHandle {
    net_thread_handle: Option<JoinHandle<()>>,
    channels: Channels,
}

pub struct Connecting {
    handle: Option<NetThreadHandle>,
    on_connect: oneshot::Receiver<Result<LoginResponse, Box<str>>>,
}

impl Connecting {
    pub fn init_connection(address: SocketAddr, username: SharedStr) -> Self {
        let (stop_command_send, stop_command_recv) = oneshot::channel();
        let (on_connect_send, on_connect_recv) = oneshot::channel();
        let (on_lost_connection_send, on_lost_connection_recv) = oneshot::channel();
        let (incoming_send, incoming_recv) = tokio::sync::mpsc::channel(64);
        let (chat_send, chat_recv) = unbounded_channel();
        let (player_state_send, player_state_recv) = unbounded_channel();

        let channels = NetSideChannels {
            incoming: incoming_send,
            chat_recv: chat_recv,
            player_state: player_state_recv,
            on_lost_connection: on_lost_connection_send,
            stop_command: stop_command_recv
        };

        Self {
            handle: Some(NetThreadHandle {
                net_thread_handle: Some(std::thread::spawn(move || {
                    network_thread::start(address, username, channels, on_connect_send)
                })),
                channels: Channels {
                    incoming: incoming_recv,
                    
                    chat: chat_send,
                    player_state: player_state_send,
                    
                    on_disconnect: on_lost_connection_recv,
                    stop_network_thread: Some(stop_command_send),
                },
            }),
            on_connect: on_connect_recv,
        }
    }

    // Returns Ok(None) until the connection has been established, after which
    // this will always return None.
    pub fn try_tick_connection(&mut self) -> Result<Option<(LoginResponse, Connection)>, Box<str>> {
        match self.on_connect.try_recv() {
            Ok(Ok(response)) => Ok(Some((
                response,
                Connection {
                    network_id_to_entity: Vec::with_capacity(512),
                    // unwrap(): safe. on_connect is oneshot, this can never be reached twice.
                    handle: self.handle.take().unwrap(),
                    closed: false,
                },
            ))),
            Ok(Err(msg)) => Err(msg),
            Err(oneshot::error::TryRecvError::Empty) => Ok(None),
            Err(e) => Err(format!("Connection failed: {e}").into_boxed_str()),
        }
    }
}

pub struct Connection {
    pub network_id_to_entity: Vec<Entity>,
    handle: NetThreadHandle,
    closed: bool,
}

impl Connection {
    pub fn closed(&self) -> bool {
        self.closed
    }

    pub fn send_disconnect(&mut self) {
        if self.closed {
            return; // guard mainly against Drop
        }
        if self
            .handle
            .channels
            .stop_network_thread
            .take()
            .unwrap()
            .send(())
            .is_err()
        {
            println!("to_net.send() failed :/");
            return;
        }

        self.closed = true;

        println!("Joining network thread. If game hangs, this is probably why");
        let start = Instant::now();
        if self
            .handle
            .net_thread_handle
            .take()
            .unwrap()
            .join()
            .is_err()
        {
            println!("Failed to join network thread");
        }
        let end = Instant::now();
        println!("join() took {}us", (end - start).as_micros());
    }

    pub fn tick(&mut self) {
        match self.handle.channels.on_disconnect.try_recv() {
            Ok(_) | Err(oneshot::error::TryRecvError::Closed) => self.closed = true,
            Err(oneshot::error::TryRecvError::Empty) => {}
        }
    }

    pub fn channels(&mut self) -> Option<&mut Channels> {
        if self.closed {
            None
        } else {
            Some(&mut self.handle.channels)
        }
    }
}

impl Drop for Connection {
    fn drop(&mut self) {
        self.send_disconnect();
    }
}

/* pub fn init(connection: Connection) -> net::Resources {
    net::Resources {
        connection,
    }
/*         Connection {
        server_address: None,
        state: ConnectionState::Disconnected,
        username: "Anonymous".to_owned(),
        network_id_to_entity: Vec::with_capacity(2048),
        handle: None, // whatever
    });
 */
    //SystemStage::single_threaded().with_system(try_reconnect)
} */

/* pub fn tick() {

}

fn try_reconnect(mut connection: ResMut<Connection>) {
    let connection = &mut *connection;
    if let ConnectionState::Connecting = connection.state {
        let handle = connection.handle.as_mut().unwrap();

        // The network thread sends a dummy message once connection is ready
        match handle.channels.on_connect.try_recv() {
            Ok(response) => {
                connection.state = ConnectionState::Connected;
            },
            Err(oneshot::error::TryRecvError::Closed) => connection.state = ConnectionState::Disconnected,
            Err(oneshot::error::TryRecvError::Empty) => { /* keep waiting */ }
        }
    }
}
 */
