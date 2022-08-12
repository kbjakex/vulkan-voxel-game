use std::net::SocketAddr;

use anyhow::bail;
use flexstr::SharedStr;
use quinn::{Endpoint, NewConnection};
use shared::{
    bits_and_bytes::ByteWriter,
    protocol::{c2s, s2c},
};
use tokio::{
    sync::{
        mpsc::{UnboundedReceiver, Sender},
        oneshot,
    },
    task,
};

use crate::networking::connection;

use anyhow::Result;

use super::{DisconnectReason, S2C};

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
    on_connect: oneshot::Sender<Result<s2c::login::LoginResponse, Box<str>>>,
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
    on_connect: oneshot::Sender<Result<s2c::login::LoginResponse, Box<str>>>,
) -> Result<()> {
    let (_endpoint, mut new_conn, response) = match try_connect(server_address, &username).await {
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
    Ok(())
}

async fn try_connect(
    server_address: SocketAddr,
    username: &SharedStr,
) -> Result<(Endpoint, NewConnection, s2c::login::LoginResponse)> {
    let endpoint = setup::make_client_endpoint().unwrap();

    println!("Connecting to {}...", server_address);
    let conn = endpoint.connect(server_address, "localhost")?.await?;

    let mut buf = [0u8; 64];
    let mut writer = ByteWriter::new(&mut buf);
    let message = c2s::login::LoginMessage { username };
    message.write(&mut writer);

    let (mut c2s_hello, mut s2c_hello) = conn.connection.open_bi().await?;
    c2s_hello.write_all(writer.bytes()).await?;
    println!("Username sent");

    let num_bytes = match s2c_hello.read(&mut buf).await? {
        Some(bytes) => bytes,
        None => {
            bail!("Error receiving login response; stream was finished?");
        }
    };

    let response = match s2c::login::LoginResponse::parse(&buf[..num_bytes]) {
        Ok(message) => message,
        Err(message_error) => bail!("Invalid login response: {:?}", message_error),
    };

    //chat!("(Server sent position {:?} and network id {})", response.position, response.network_id);

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
