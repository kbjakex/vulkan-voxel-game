use flexstr::{SharedStr, ToSharedStr};
use quinn::{NewConnection, VarInt};
use shared::{protocol::{NetworkId, PROTOCOL_MAGIC, PROTOCOL_VERSION}};
use tokio::{
    sync::{
        mpsc::unbounded_channel, oneshot,
    },
    task,
};

use crate::{networking::{client_connection::receive_bytes, LoginResponse}, net::PlayerChannels};

use super::{client_connection, PlayersChanged, network_thread::NetSideChannels};

pub(super) async fn login(
    mut connection: NewConnection,
    channels: NetSideChannels
) -> anyhow::Result<()> {
    println!("Trying to accept uni stream...");
    let (mut hello_send, mut hello_recv) = connection.bi_streams.next().await.unwrap()?;

    let mut recv_buf = Vec::new();
    let mut reader = receive_bytes(&mut hello_recv, &mut recv_buf).await?;
    println!("Received login message! Length: {}", reader.bytes_remaining());
    
    if reader.bytes_remaining() < 6 // magic + protocol ver + username length + username >= 6
        || reader.read_u16() != PROTOCOL_MAGIC 
        || reader.read_u16() != PROTOCOL_VERSION 
    { 
        connection.connection.close(VarInt::from_u32(1), b"Invalid login request");
        anyhow::bail!("Invalid login request");
    }
    
    let username_len = reader.read_u8() as usize;
    let username = reader.read_str(username_len).to_shared_str();
    if username.len() < 3 {
        connection.connection.close(VarInt::from_u32(2), b"Username too short");
        anyhow::bail!("Username too short");
    }

    println!("Username: {username}. Generating network ID...");

    let (id_send, id_recv) = oneshot::channel();
    channels.player_join_send
        .send(PlayersChanged::LoginRequest { channel: id_send, username: username.clone() })
        .unwrap();
        
    let (network_id, login_response) = id_recv.await?;
    match login_response {
        LoginResponse::Success(response_bytes) => hello_send.write_all(&response_bytes).await?,
        LoginResponse::Denied(reason) => {
            connection.connection.close(VarInt::from_u32(2), reason);
            anyhow::bail!("Invalid login request");
        },
    }
    hello_send.finish().await?;

    task::spawn(async move {
        if let Err(e) = client_connection(connection, username, network_id, channels).await {
            println!("Error in client connection: {e}");
        }
    });
    Ok(())
}

async fn client_connection(
    mut connection: NewConnection,
    username: SharedStr,
    network_id: NetworkId,
    channels: NetSideChannels
) -> anyhow::Result<()> {
    let (chat_send_main, chat_recv_self) = unbounded_channel(); // c -> s
    let (entity_state_send, entity_state_recv) = unbounded_channel(); // s -> c

    let (chat_recv_driver, chat_send_driver) = {
        let (outgoing, mut incoming) = connection.bi_streams.next().await.unwrap()?;

        // Read the byte that was used to open the channel
        incoming.read_exact(&mut [0u8]).await?;

        let chat_recv_driver = task::spawn(client_connection::chat::recv_driver(
            incoming,
            username.clone(),
            network_id,
            channels.chat_send,
        ));
        let chat_send_driver = task::spawn(client_connection::chat::send_driver(
            outgoing,
            chat_recv_self,
        ));

        (chat_recv_driver, chat_send_driver)
    };

    let player_state_recv_driver = {
        let mut stream = connection.uni_streams.next().await.unwrap()?;
        stream.read_exact(&mut [0u8]).await?;

        task::spawn(client_connection::player_state::recv_driver(network_id, stream, channels.player_state_send))
    };

    let entity_state_send_driver = {
        let mut stream = connection.connection.open_uni().await?;
        stream.write_all(&[0u8]).await?;

        task::spawn(client_connection::entity_state::send_driver(stream, entity_state_recv))
    };

    // Keep at the end so that Disconnect is definitely sent (no more early exits).
    // Disconnect must be sent to avoid leaking network ids
    channels.player_join_send
        .send(PlayersChanged::Connected {
            username: username.clone(),
            network_id,
            channels: PlayerChannels {
                chat_send: chat_send_main,
                entity_state: entity_state_send,
            }
        })
        .unwrap();

    tokio::select!(
        biased;
        _ = chat_recv_driver => {println!("chat::recv_driver returned")},
        _ = chat_send_driver => {println!("chat::send_driver returned")},
        _ = player_state_recv_driver => {println!("player_state::recv_driver returned")},
        _ = entity_state_send_driver => {println!("entity_state::send_driver returned")},
    );

    channels.player_join_send
        .send(PlayersChanged::Disconnect { network_id })
        .unwrap();

    println!("Client with username \"{}\" disconnected", username);
    Ok(())
}
