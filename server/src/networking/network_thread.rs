use std::net::SocketAddr;

use anyhow::Result;
use flexstr::SharedStr;
use glam::{Vec3, Vec2};
use quinn::Incoming;
use tokio::{
    sync::{
        mpsc::UnboundedSender,
        oneshot,
    },
    task,
};

use crate::{networking::login, components::NetworkId};

use super::PlayersChanged;

#[derive(Debug)]
pub struct PlayerStateMsg {
    pub tick: u32,
    pub delta_pos: Option<Vec3>,
    pub delta_yaw_pitch: Option<Vec2>,
}
#[derive(Clone)]
pub struct NetSideChannels {
    pub chat_send: UnboundedSender<(NetworkId, SharedStr)>,
    pub player_join_send: UnboundedSender<PlayersChanged>,
    pub player_state_send: UnboundedSender<(NetworkId, PlayerStateMsg)>
}

#[tokio::main(flavor = "multi_thread", worker_threads = 3)]
pub async fn start(
    tx: oneshot::Sender<bool>,
    channels: NetSideChannels,
) {
    let incoming = match setup::make_server_endpoint("0.0.0.0:29477".parse().unwrap()) {
        Ok(incoming) => incoming,
        Err(e) => {
            println!("Failed to create server endpoint! Error: {}", e);
            tx.send(false).unwrap();
            return;
        }
    };
    tx.send(true).unwrap(); // unwrap(): crashing is probably not a terrible solution on failure

    poll_new_connections(incoming, channels).await;
    println!("Network thread terminating...");
}

async fn poll_new_connections(
    mut incoming: Incoming,
    channels: NetSideChannels
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

        let channels = channels.clone();
        task::spawn(async move {
            if let Err(e) = login::login(new_conn, channels).await {
                println!("Login attempt failed: {e}");
            }
        });
    }
}

mod setup {
    use std::sync::Arc;

    use quinn::{Endpoint, Incoming, ServerConfig};

    use super::*;

    pub fn make_server_endpoint(bind_addr: SocketAddr) -> Result<Incoming> {
        let (server_config, _) = configure_server()?;
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
