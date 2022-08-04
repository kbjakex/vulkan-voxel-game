use std::net::{SocketAddr, Ipv4Addr};

use anyhow::Result;
use flexstr::SharedStr;
use glam::Vec3;
use quinn::{Incoming, SendStream};
use tokio::{
    sync::{
        mpsc::UnboundedSender,
        oneshot,
    },
    task,
};

use crate::{networking::login};

use super::PlayersChanged;

#[derive(Debug)]
pub struct PlayerStateMsg {
    pub delta_pos: Option<Vec3>,
}
#[derive(Clone)]
pub struct NetSideChannels {
    pub chat_send: UnboundedSender<(shared::protocol::NetworkId, SharedStr)>,
    pub player_join_send: UnboundedSender<PlayersChanged>,
    pub player_state_send: UnboundedSender<(shared::protocol::NetworkId, PlayerStateMsg)>
}

#[tokio::main(flavor = "current_thread")]
pub async fn start(
    tx: oneshot::Sender<bool>,
    channels: NetSideChannels,
) {
    let incoming = match setup::make_server_endpoint(SocketAddr::new(Ipv4Addr::LOCALHOST.into(), 29477)) {//"65.108.78.237:29477") {
        Ok(incoming) => incoming,
        Err(e) => {
            println!("Failed to create server endpoint! Error: {}", e);
            tx.send(false).unwrap();
            return;
        }
    };
    tx.send(true).unwrap(); // unwrap(): crashing is probably not a terrible solution on failure

    let (conn_sender, conn_receiver) = tokio::sync::mpsc::unbounded_channel();

    poll_new_connections(incoming, channels, conn_sender).await;
    println!("Network thread terminating...");
}

async fn poll_new_connections(
    mut incoming: Incoming,
    channels: NetSideChannels,
    out_channel: UnboundedSender<(u32, SendStream)>,
) {
    println!("Now polling for connections!");
    while let Some(connecting) = incoming.next().await {
        println!("Received connection attempt, resolving...");
        let new_conn = match connecting.await {
            Ok(conn) => conn,
            Err(e) => {
                println!("Connection failed: {}", e);
                continue;
            }
        };

        println!("Connection established!");

        task::spawn(login::login(
            new_conn,
            channels.clone(),
            out_channel.clone(),
        ));
    }
}

mod setup {
    use std::sync::Arc;

    use quinn::{Endpoint, Incoming, ServerConfig};

    use super::*;

    pub fn make_server_endpoint(bind_addr: SocketAddr) -> Result<Incoming> {
        let (server_config, server_cert) = configure_server()?;
        let (endpoint, incoming) = Endpoint::server(server_config, bind_addr)?;

        println!(
            "Network thread listening for connections on {}",
            endpoint.local_addr()?
        );
        Ok(incoming)
    }

    /// Returns default server configuration along with its certificate.
    #[allow(clippy::field_reassign_with_default)] // https://github.com/rust-lang/rust-clippy/issues/6527
    fn configure_server() -> Result<(ServerConfig, Vec<u8>)> {
        let cert = rcgen::generate_simple_self_signed(vec!["localhost".into()]).unwrap();
        let cert_der = cert.serialize_der().unwrap();
        let priv_key = cert.serialize_private_key_der();
        let priv_key = rustls::PrivateKey(priv_key);
        let cert_chain = vec![rustls::Certificate(cert_der.clone())];

        let mut server_config = ServerConfig::with_single_cert(cert_chain, priv_key)?;
        Arc::get_mut(&mut server_config.transport)
            .unwrap()
            .keep_alive_interval(Some(std::time::Duration::from_millis(6000)));

        Ok((server_config, cert_der))
    }
}
