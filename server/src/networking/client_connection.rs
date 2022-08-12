use quinn::{RecvStream, SendStream};
use shared::bits_and_bytes::ByteReader;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

use anyhow::Result;

pub async fn receive_bytes<'a>(stream: &mut RecvStream, buf: &'a mut Vec<u8>) -> anyhow::Result<ByteReader<'a>> {
    let mut header = [0u8; 2];
    stream.read_exact(&mut header[0..2]).await?;

    let mut length = header[0] as usize;
    if length > 127 {
        length = length - 128 + ((header[1] as usize) << 7);
    }

    if length == 0 {
        anyhow::bail!("Received zero-length message! This is a client-side error.");
    }
    
    buf.resize(length, 0);
    let slice = if length > 127 {
        &mut buf[..length]
    } else {
        buf[0] = header[1];
        &mut buf[1..length]
    };

    stream.read_exact(slice).await?;
    Ok(ByteReader::new(&mut buf[..]))
}

pub(super) mod chat {
    use flexstr::SharedStr;
    use shared::{protocol::NetworkId, bits_and_bytes::ByteWriter};

    use super::*;

    pub async fn recv_driver(
        mut incoming: RecvStream,
        username: SharedStr,
        id: NetworkId,
        to_server: UnboundedSender<(NetworkId, SharedStr)>,
    ) -> Result<()> {
        println!("chat::recv_driver ready");

        let mut buf = Vec::new();
        loop {
            let mut stream = receive_bytes(&mut incoming, &mut buf).await?;
            
            let message = username.clone() + ": " + stream.read_str(stream.bytes_remaining());
            println!("Received '{}' (length {})", message, message.len());
            let _ = to_server.send((id, message));
        }
    }

    pub async fn send_driver(
        mut outgoing: SendStream,
        mut messages: UnboundedReceiver<SharedStr>,
    ) -> Result<()> {
        println!("chat::send_driver ready");
        let mut buf = [0u8; 512];
        while let Some(message) = messages.recv().await {
            debug_assert!(message.len() < buf.len(), "chat::send_driver: message too long! ({}/{} bytes)", message.len(), buf.len());

            let mut writer = ByteWriter::new_for_message(&mut buf);
            writer.write(message.as_bytes());
            writer.write_message_len();

            outgoing.write_all(&writer.bytes()).await?;
        }
        Ok(())
    }
}

pub(super) mod player_state {
    use glam::{vec3, vec2};
    use shared::{protocol::{NetworkId, decode_angle_rad, decode_velocity}};

    use crate::networking::network_thread::PlayerStateMsg;

    use super::*;

    pub async fn recv_driver(
        id: NetworkId,
        mut incoming: RecvStream,
        to_server: UnboundedSender<(NetworkId, PlayerStateMsg)>,
    ) -> Result<()> {
        println!("player_state::recv_driver ready");
        let mut buf = Vec::new();
        loop {
            let mut stream = receive_bytes(&mut incoming, &mut buf).await?;        

            let mut msg = PlayerStateMsg {
                tick: stream.read_u32(),
                delta_pos: None,
                delta_yaw_pitch: None,
            };

            let mask = stream.read_u8() as u32;            
            
            if (mask & 0x1) != 0 {    
                let dx = decode_velocity(stream.read_u16() as u32);
                let dy = decode_velocity(stream.read_u16() as u32);
                let dz = decode_velocity(stream.read_u16() as u32);
                msg.delta_pos = Some(vec3(dx, dy, dz));
            }

            if (mask & 0x2) != 0 {
                let delta_yaw = decode_angle_rad(stream.read_u16() as u16);
                let delta_pitch = decode_angle_rad(stream.read_u16() as u16);
                msg.delta_yaw_pitch = Some(vec2(delta_yaw, delta_pitch));
            }

            let _ = to_server.send((id, msg));
        }
    }
}

pub(super) mod entity_state {
    use super::*;

    pub async fn send_driver(
        mut outgoing: SendStream,
        mut messages: UnboundedReceiver<Box<[u8]>>,
    ) -> Result<()> {
        println!("entity_state::send_driver ready");
        while let Some(message) = messages.recv().await {
            outgoing.write_all(&message).await?;
        }
        Ok(())
    }
}