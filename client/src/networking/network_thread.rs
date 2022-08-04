use std::net::{SocketAddr, Ipv4Addr};

use anyhow::bail;
use flexstr::SharedStr;
use quinn::{Endpoint, NewConnection, SendStream};
use shared::{protocol::{c2s, s2c}, bits_and_bytes::ByteWriter};
use tokio::{
    sync::{
        mpsc::{UnboundedReceiver, UnboundedSender},
        oneshot,
    },
    task,
};

use crate::{chat::Chat, networking::connection};

use anyhow::Result;

pub struct NetSideChannels {
    pub chat: UnboundedReceiver<SharedStr>,
    pub entity_state: UnboundedSender<Vec<u8>>,
    pub player_state: UnboundedReceiver<Vec<u8>>,
    pub disconnect: oneshot::Receiver<()>,
    pub on_lost_connection: oneshot::Sender<()>,
}

pub fn start(
    server_address: SocketAddr,
    username: SharedStr,
    channels: NetSideChannels,
    on_connect: oneshot::Sender<Result<s2c::login::LoginResponse, Box<str>>>,
) {
    if let Err(e) = start_inner(server_address, username, channels, on_connect) {
        Chat::write(format!("Error in network thread: {}", e), 0xFF_22_22_FF);
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
            on_connect.send(Err(format!("Connection failed: {e}").into_boxed_str()));
            //Chat::write(format!("Connection failed! Reason: {}", e), 0xFF_22_22_FF);
            return Ok(());
        }
    };

    dbg![ new_conn.connection.max_datagram_size() ];

    /* Chat::write("Connected!".to_owned(), 0x22_FF_22_FF); */

    let (mut chat_send, chat_recv) = new_conn.connection.open_bi().await?;
    chat_send.write(&[0]).await?; // open up the channel on the server side as well
    let chat_fut_1 = task::spawn(connection::chat::recv_driver(chat_recv));
    let chat_fut_2 = task::spawn(connection::chat::send_driver(chat_send, channels.chat));

    let mut player_state_send = new_conn.connection.open_uni().await?;
    player_state_send.write(&[0]).await?;
    let player_fut = task::spawn(connection::player_state::send_driver(
        player_state_send,
        channels.player_state,
    ));

    let entity_state_recv = new_conn.uni_streams.next().await.unwrap()?;
    let entity_fut = task::spawn(connection::entity_state::recv_driver(
        entity_state_recv,
        channels.entity_state,
    ));

    let disconnect = channels.disconnect;
    let temp_fut = async {
        println!("Temp fut starting");
        disconnect.await.unwrap();
        println!("Temp fut finished1");
    };

    if on_connect.send(Ok(response)).is_err() {
        println!("Main thread dropped on_connect channel");
        return Ok(());
    }

    tokio::select!(
        _ = chat_fut_1 => {println!("chat::recv_driver returned");},
        _ = chat_fut_2 => {println!("chat::send_driver returned");}
        _ = entity_fut => {println!("entity_state::recv_driver returned");}
        _ = player_fut => {println!("player_state::send_driver returned");}
        _ = temp_fut => {}
    );

    println!("Stopping network thread");
    Ok(())
}

async fn try_connect(
    server_address: SocketAddr,
    username: &SharedStr,
) -> Result<(Endpoint, NewConnection, s2c::login::LoginResponse)> {
    let endpoint = setup::make_client_endpoint("0.0.0.0:0".parse()? /*, &[&server_cert]*/).unwrap();

    Chat::write(
        format!("Connecting to {}...", server_address),
        0xFF_FF_FF_FF,
    );
    let conn = endpoint
        .connect(server_address, "localhost")?
        .await?;

    let mut buf = [0u8; 64];
    let mut writer = ByteWriter::new(&mut buf);
    let message = c2s::login::LoginMessage { username };
    message.write(&mut writer);
    let length = writer.bytes_written() as usize;

    let (mut c2s_hello, mut s2c_hello) = conn.connection.open_bi().await?;
    Chat::write("Sending username...".to_owned(), 0xFF_FF_FF_FF);
    c2s_hello.write_all(&buf[0..length]).await?;
    println!("Username sent");

    let num_bytes = match s2c_hello.read(&mut buf).await {
        Ok(Some(bytes)) => bytes,
        Ok(None) => {
            bail!("Error receiving login response; stream was finished?");
        }
        Err(e) => {
            bail!("Error receiving login response: {}", e);
        }
    };

    let response = match s2c::login::LoginResponse::parse(&buf[..num_bytes]) {
        Ok(message) => message,
        Err(message_error) => bail!("Invalid login response: {:?}", message_error),
    };

    //chat!("(Server sent position {:?} and network id {})", response.position, response.network_id);

    Ok((endpoint, conn, response))
}

async fn process_outgoing(
    mut to_server: SendStream,
    mut from_main: UnboundedReceiver<Vec<u8>>,
) -> Result<()> {
    let mut outgoing_buffer: Vec<u8> = Vec::new();
    outgoing_buffer.resize(2048, 0);

    while let Some(msg) = from_main.recv().await {
        to_server.write_all(&msg).await?;
    }
    Ok(())
}

mod setup {
    use std::{error::Error, net::SocketAddr, sync::Arc};

    use quinn::{ClientConfig, Endpoint, TransportConfig};

    pub(super) fn make_client_endpoint(
        bind_addr: SocketAddr,
        /*server_certs: &[&[u8]],*/
    ) -> Result<Endpoint, Box<dyn Error>> {
        let client_cfg = insecure(); //configure_client(/*server_certs*/)?;
        let mut endpoint = Endpoint::client(bind_addr)?;
        endpoint.set_default_client_config(client_cfg);
        Ok(endpoint)
    }

    struct SkipServerVerification;

    impl SkipServerVerification {
        fn new() -> Arc<Self> {
            Arc::new(Self)
        }
    }

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

    fn insecure() -> ClientConfig {
        let crypto = rustls::ClientConfig::builder()
            .with_safe_defaults()
            .with_custom_certificate_verifier(SkipServerVerification::new())
            .with_no_client_auth();

        let mut tconf = TransportConfig::default();
        //tconf.max_idle_timeout(None);

        let mut client_config = ClientConfig::new(std::sync::Arc::new(crypto));
        /* client_config.transport_config(Arc::new(tconf)); */

        client_config
    }

    #[allow(unused)]
    fn configure_client(/*server_certs: &[&[u8]]*/) -> Result<ClientConfig, Box<dyn Error>> {
        let mut certs = rustls::RootCertStore::empty();
        /* for cert in server_certs {
            certs.add(&rustls::Certificate(cert.to_vec()))?;
        } */

        let mut client_config = ClientConfig::with_root_certificates(certs);
        let mut tconf = TransportConfig::default();
        tconf.max_idle_timeout(None);
        client_config.transport_config(Arc::new(tconf));

        Ok(client_config)
    }
}
