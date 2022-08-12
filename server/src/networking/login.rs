use flexstr::{SharedStr, ToSharedStr};
use glam::Vec3;
use quinn::{ConnectionError, NewConnection};
use shared::{protocol::{c2s, s2c, NetworkId}, bits_and_bytes::ByteWriter};
use tokio::{
    sync::{
        mpsc::{self, unbounded_channel},
    },
    task,
};

use crate::components::net::PlayerConnection;

use super::{client_connection, PlayersChanged, network_thread::NetSideChannels};

pub(super) async fn login(
    mut connection: NewConnection,
    channels: NetSideChannels
) {
    println!("Trying to accept uni stream...");
    let (mut s2c_hello, mut c2s_hello) = match connection.bi_streams.next().await.unwrap() {
        Ok(stream) => stream,
        Err(ConnectionError::TimedOut) => {
            println!("Connection timed out");
            return;
        }
        Err(e) => {
            println!("Connection failed: {}", e);
            return;
        }
    };

    let mut buf = [0u8; 64];
    let num_bytes = match c2s_hello.read(&mut buf).await {
        Ok(Some(bytes)) => bytes,
        Ok(None) => {
            println!("Error receiving login message; stream was finished?");
            return;
        }
        Err(e) => {
            println!("Error receiving login message: {}", e);
            return;
        }
    };

    println!("Received login message! Length: {}", num_bytes);

    let message = match c2s::login::LoginMessage::parse(&buf[..num_bytes]) {
        Ok(message) => message,
        Err(message_error) => {
            println!("Invalid login Message: {:?}", message_error);
            return;
        }
    };

    let username = message.username.to_shared_str();
    println!("Username: {username}. Generating network ID...");

    let (id_send, mut id_recv) = mpsc::channel(1);
    channels.player_join_send
        .send(PlayersChanged::NetworkIdRequest { channel: id_send })
        .unwrap();
    let network_id = id_recv.recv().await.unwrap();

    println!(
        "Login received! Username: {}, id: {}",
        message.username, network_id
    );

    let message = s2c::login::LoginResponse {
        network_id,
        position: Vec3::ZERO,
        world_seed: 0,
    };

    let mut writer = ByteWriter::new(&mut buf);
    message.write(&mut writer);
    let size = writer.bytes_written();

    println!("Sending {} bytes", size);

    if let Err(e) = s2c_hello.write_all(&buf[0..size]).await {
        println!("Failed to send login response: {}", e);
    }

    println!("Sent.");

    task::spawn(setup_client(connection, username, network_id, channels));
}

async fn setup_client(
    mut connection: NewConnection,
    username: SharedStr,
    network_id: NetworkId,
    channels: NetSideChannels
) {
    let (chat_send_main, chat_recv_self) = unbounded_channel(); // c -> s
    let (entity_state_send, entity_state_recv) = unbounded_channel(); // s -> c

    let (chat_fut_1, chat_fut_2) = {
        println!("Waiting for bistream... (chat)");

        let (outgoing, mut incoming) = match connection.bi_streams.next().await.unwrap() {
            Ok(streams) => streams,
            Err(ConnectionError::TimedOut) => {
                println!("Client didn't open chat bistream!");
                return;
            }
            Err(e) => {
                println!("Client connection errored out after handshake: {}", e);
                return;
            }
        };

        // Read the byte that was used to open the channel
        if incoming.read_exact(&mut [0u8]).await.is_err() {
            println!("Random error #1425971");
            return;
        };

        println!("Bistream opened");

        let chat_fut_1 = task::spawn(client_connection::chat::recv_driver(
            incoming,
            username.clone(),
            network_id,
            channels.chat_send,
        ));
        let chat_fut_2 = task::spawn(client_connection::chat::send_driver(
            outgoing,
            chat_recv_self,
        ));

        (chat_fut_1, chat_fut_2)
    };

    let player_state_fut = {
        println!("Waiting for unistream... (player state)");

        let mut stream = match connection.uni_streams.next().await.unwrap() {
            Ok(stream) => stream,
            Err(ConnectionError::TimedOut) => {
                println!("Client didn't open player state unistream!");
                return;
            },
            Err(e) => {
                println!("Client connection errored out after chat stream: {}", e);
                return;
            }
        };

        if stream.read_exact(&mut [0u8]).await.is_err() {
            println!("Random error #14824018");
            return;
        }

        println!("Unistream opened!");

        task::spawn(client_connection::player_state::recv_driver(network_id, stream, channels.player_state_send))
    };

    let entity_state_fut = {
        println!("Opening unistream (entity state)");

        let mut stream = match connection.connection.open_uni().await {
            Ok(stream) => stream,
            Err(ConnectionError::TimedOut) => {
                println!("Client didn't accept entity state unistream!");
                return;
            },
            Err(e) => {
                println!("Client connection errored out after player state stream: {}", e);
                return;
            }
        };

        if stream.write_all(&[0u8]).await.is_err() {
            println!("Random error #12471982");
            return;
        }

        println!("Unistream opened!");

        task::spawn(client_connection::entity_state::send_driver(stream, entity_state_recv))
    };

    // Keep at the end so that Disconnect is definitely sent (no more early exits).
    // Disconnect must be sent to avoid leaking network ids
    channels.player_join_send
        .send(PlayersChanged::Connected {
            username: username.clone(),
            network_id,
            channels: PlayerConnection {
                chat_send: chat_send_main,
                entity_state: entity_state_send,
            }
        })
        .unwrap();

    tokio::select!(
        biased;
        _ = chat_fut_1 => {println!("chat::recv_driver returned")},
        _ = chat_fut_2 => {println!("chat::send_driver returned")},
        _ = player_state_fut => {println!("player_state::recv_driver returned")},
        _ = entity_state_fut => {println!("entity_state::send_driver returned")},
    );

    channels.player_join_send
        .send(PlayersChanged::Disconnect { network_id })
        .unwrap();

    println!("Client with username \"{}\" disconnected", username);
}

/* async fn setup_chat(
    connection: &mut NewConnection,
    username: SharedStr,
    chat_send: UnboundedSender<String>,
    chat_recv: UnboundedReceiver<String>,
) -> Result<(JoinHandle<Result<()>>, JoinHandle<Result<()>>)> {
    println!("Waiting for bistream...");

    let (outgoing, incoming) = match connection.bi_streams.next().await.unwrap() {
        Ok(streams) => streams,
        Err(ConnectionError::TimedOut) => {
            bail!("Client timed out after handshake before properly connecting")
        }
        Err(e) => bail!("Client connection errored out after handshake: {}", e),
    };

    println!("Bistream opened");

    let chat_fut_1 = task::spawn(client_connection::chat::recv_driver(
        incoming, username, chat_send,
    ));
    let chat_fut_2 = task::spawn(client_connection::chat::send_driver(outgoing, chat_recv));

    Ok((chat_fut_1, chat_fut_2))
}
 */