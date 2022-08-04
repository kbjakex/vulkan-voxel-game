use std::{net::SocketAddr, thread::JoinHandle, time::Instant};

use flexstr::SharedStr;
use hecs::Entity;
use shared::protocol::{NetworkId, s2c::login::LoginResponse};
use tokio::sync::{mpsc::{
    unbounded_channel, UnboundedReceiver, UnboundedSender,
}, oneshot};

use self::network_thread::NetSideChannels;

pub mod connection;
mod login;
mod network_thread;
pub mod state_sync;
pub mod interpolation;

pub struct EntityState {
    pub id: NetworkId,
    
}

#[derive(PartialEq, Eq, Clone, Copy)]
pub enum ConnectionState {
    Disconnected,
    Connecting,
    Connected,
}

pub struct Channels {
    pub chat_send: UnboundedSender<SharedStr>,

    pub entity_state_recv: UnboundedReceiver<Vec<u8>>,
    pub player_state_send: UnboundedSender<Vec<u8>>,

    pub on_lost_connection: oneshot::Receiver<()>,
    pub disconnect: Option<oneshot::Sender<()>>,
}

struct NetThreadHandle {
    net_thread_handle: Option<JoinHandle<()>>,
    channels: Channels
}

pub struct Connecting {
    handle: Option<NetThreadHandle>,
    on_connect: oneshot::Receiver<Result<LoginResponse, Box<str>>>,
}

impl Connecting {
    pub fn init_connection(address: SocketAddr, username: SharedStr) -> Self {
        let (disconnect_send, disconnect_recv) = oneshot::channel();
        let (on_connect_send, on_connect_recv) = oneshot::channel();
        let (on_lost_connection_send, on_lost_connection_recv) = oneshot::channel();
        let (chat_send, chat_recv) = unbounded_channel();
        let (entity_state_send, entity_state_recv) = unbounded_channel();
        let (player_state_send, player_state_recv) = unbounded_channel();

        let channels = NetSideChannels {
            chat: chat_recv,
            entity_state: entity_state_send,
            player_state: player_state_recv,
            disconnect: disconnect_recv,
            on_lost_connection: on_lost_connection_send
        };

        Self {
            handle: Some(NetThreadHandle {
                net_thread_handle: Some(std::thread::spawn(move || {
                    network_thread::start(address, username, channels, on_connect_send)
                })),
                channels: Channels {
                    disconnect: Some(disconnect_send),
                    on_lost_connection: on_lost_connection_recv,
                    chat_send,
                    entity_state_recv,
                    player_state_send,
                }
            }),
            on_connect: on_connect_recv
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
                    closed: false
                }
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
    closed: bool
}

impl Connection {
    pub fn closed(&self) -> bool {
        self.closed
    }

    pub fn send_disconnect(&mut self) {
        if self.closed {
            return; // guard mainly against Drop
        }
        if self.handle.channels.disconnect.take().unwrap().send(()).is_err() {
            println!("to_net.send() failed :/");
            return;
        }

        self.closed = true;

        println!("Joining network thread. If game hangs, this is probably why");
        let start = Instant::now();
        if self.handle.net_thread_handle.take().unwrap().join().is_err() {
            println!("Failed to join network thread");
        }
        let end = Instant::now();
        println!("join() took {}us", (end-start).as_micros());
    }

    pub fn tick(&mut self) {
        match self.handle.channels.on_lost_connection.try_recv() {
            Ok(()) | Err(oneshot::error::TryRecvError::Closed) => self.closed = true,
            Err(oneshot::error::TryRecvError::Empty) => {},
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