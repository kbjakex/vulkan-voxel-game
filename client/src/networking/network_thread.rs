use std::net::SocketAddr;

use flexstr::SharedStr;
use glam::{Vec2, Vec3};
use quinn::{Endpoint, NewConnection, VarInt};
use shared::{
    bits_and_bytes::ByteWriter, protocol::NetworkId
};
use tokio::{
    sync::{
        mpsc::{UnboundedReceiver, Sender},
        oneshot,
    },
    task,
};

use crate::networking::connection::{self, receive_bytes};

use anyhow::Result;

use super::{DisconnectReason, S2C, LoginResponse};

pub struct NetSideChannels {
    pub incoming: Sender<S2C>,
    pub chat_recv: UnboundedReceiver<SharedStr>,
    pub player_state: UnboundedReceiver<Box<[u8]>>,
    pub on_lost_connection: oneshot::Sender<DisconnectReason>,

    pub stop_command: oneshot::Receiver<()>,
}

pub fn start(
    server_address: SocketAddr,
    username: SharedStr,
    channels: NetSideChannels,
    on_connect: oneshot::Sender<Result<LoginResponse, Box<str>>>,
) {
    if let Err(e) = start_inner(server_address, username, channels, on_connect) {
        println!("Error in network thread: {}", e);
    }
}

#[tokio::main(flavor = "current_thread")]
async fn start_inner(
    server_address: SocketAddr,
    username: SharedStr,
    channels: NetSideChannels,
    on_connect: oneshot::Sender<Result<LoginResponse, Box<str>>>,
) -> Result<()> {
    let (endpoint, mut new_conn, response) = match try_connect(server_address, &username).await {
        Ok(tuple) => tuple,
        Err(e) => {
            println!("Connection failed: {e}");
            let _ = on_connect.send(Err(format!("Connection failed: {e}").into_boxed_str()));
            return Ok(());
        }
    };

    dbg![new_conn.connection.max_datagram_size()];

    let (mut chat_send, chat_recv) = new_conn.connection.open_bi().await?;
    chat_send.write(&[0]).await?; // open up the channel on the server side as well
    let chat_fut_1 = task::spawn(connection::chat::recv_driver(chat_recv, channels.incoming.clone()));
    let chat_fut_2 = task::spawn(connection::chat::send_driver(chat_send, channels.chat_recv));

    let mut player_state_send = new_conn.connection.open_uni().await?;
    player_state_send.write(&[0]).await?;
    let player_fut = task::spawn(connection::player_state::send_driver(
        player_state_send,
        channels.player_state,
    ));

    let mut entity_state_recv = new_conn.uni_streams.next().await.unwrap()?;
    entity_state_recv.read_exact(&mut [0u8]).await?; // Read the byte used to open the channel
    let entity_fut = task::spawn(connection::entity_state::recv_driver(
        entity_state_recv,
        channels.incoming.clone(),
    ));

    let disconnect = channels.stop_command;

    if on_connect.send(Ok(response)).is_err() {
        println!("Main thread dropped on_connect channel");
        return Ok(());
    }

    tokio::select!(
        _ = chat_fut_1 => {println!("chat::recv_driver returned");},
        _ = chat_fut_2 => {println!("chat::send_driver returned");}
        _ = entity_fut => {println!("entity_state::recv_driver returned");}
        _ = player_fut => {println!("player_state::send_driver returned");}
        _ = disconnect => {}
    );

    println!("Stopping network thread");
    endpoint.close(VarInt::from_u32(1), &[]);
    endpoint.wait_idle().await;
    println!("Network thread stopped");
    Ok(())
}

async fn try_connect(
    server_address: SocketAddr,
    username: &SharedStr,
) -> Result<(Endpoint, NewConnection, LoginResponse)> {
    let endpoint = setup::make_client_endpoint().unwrap();

    println!("Connecting to {}...", server_address);
    let conn = endpoint.connect(server_address, "localhost")?.await?;

    let mut buf = [0u8; 256];
    let mut writer = ByteWriter::new_for_message(&mut buf);
    writer.write_u16(shared::protocol::PROTOCOL_MAGIC);
    writer.write_u16(shared::protocol::PROTOCOL_VERSION);
    writer.write_u8(username.len() as u8);
    writer.write(username.as_str().as_bytes());
    writer.write_message_len();

    let (mut hello_send, mut hello_recv) = conn.connection.open_bi().await?;
    hello_send.write_all(writer.bytes()).await?;

    let mut recv_buf = Vec::new();
    let mut reader = receive_bytes(&mut hello_recv, &mut recv_buf).await?;
    if reader.bytes_remaining() < 30 {
        anyhow::bail!("Invalid login response from server, got only {} bytes", reader.bytes_remaining());
    }

    let response = LoginResponse {
        nid: NetworkId::from_raw(reader.read_u16()),
        position: Vec3 {
            x: reader.read_f32(),
            y: reader.read_f32(),
            z: reader.read_f32(),
        },
        head_rotation: Vec2 {
            x: reader.read_f32(), // Yaw
            y: reader.read_f32(), // Pitch
        },
        world_seed: reader.read_u64(), // World seed
    };

    Ok((endpoint, conn, response))
}

mod setup {
    use std::{error::Error, sync::Arc};

    use quinn::{ClientConfig, Endpoint};

    pub(super) fn make_client_endpoint() -> Result<Endpoint, Box<dyn Error>> {
        let mut endpoint = Endpoint::client("[::]:0".parse()?)?;
        let crypto = rustls::ClientConfig::builder()
            .with_safe_defaults()
            .with_custom_certificate_verifier(Arc::new(SkipServerVerification))
            .with_no_client_auth();
        endpoint.set_default_client_config(ClientConfig::new(std::sync::Arc::new(crypto)));
        Ok(endpoint)
    }

    struct SkipServerVerification;

    impl rustls::client::ServerCertVerifier for SkipServerVerification {
        fn verify_server_cert(
            &self,
            _end_entity: &rustls::Certificate,
            _intermediates: &[rustls::Certificate],
            _server_name: &rustls::ServerName,
            _scts: &mut dyn Iterator<Item = &[u8]>,
            _ocsp_response: &[u8],
            _now: std::time::SystemTime,
        ) -> Result<rustls::client::ServerCertVerified, rustls::Error> {
            Ok(rustls::client::ServerCertVerified::assertion())
        }
    }

}
