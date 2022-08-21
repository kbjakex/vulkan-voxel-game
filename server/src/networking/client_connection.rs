use quinn::{RecvStream, SendStream};
use shared::bits_and_bytes::ByteReader;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

use anyhow::Result;

pub async fn receive_bytes<'a>(stream: &mut RecvStream, buf: &'a mut Vec<u8>, max_length: usize) -> anyhow::Result<ByteReader<'a>> {
    let mut header = [0u8; 2];
    stream.read_exact(&mut header[0..2]).await?;

    let mut length = header[0] as usize;
    if length > 127 {
        length = length - 128 + ((header[1] as usize) << 7);
    }

    if length == 0 {
        anyhow::bail!("Received zero-length message! This is a client-side error.");
    }
    if length >= max_length {
        anyhow::bail!("Message too long ({length} / {max_length})");
    }

    //println!("Received {length} bytes");
    
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
        //println!("chat::recv_driver ready");

        let mut buf = Vec::new();
        loop {
            let mut stream = receive_bytes(&mut incoming, &mut buf, 600).await?;
            
            let message = username.clone() + ": " + stream.read_str(stream.bytes_remaining());
            //println!("Received '{}' (length {})", message, message.len());
            let _ = to_server.send((id, message));
        }
    }

    pub async fn send_driver(
        mut outgoing: SendStream,
        mut messages: UnboundedReceiver<SharedStr>,
    ) -> Result<()> {
        //println!("chat::send_driver ready");
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
    use quinn::Datagrams;
    use shared::{protocol::{NetworkId, decode_angle_rad, decode_velocity}, bits_and_bytes::BitReader};

    use crate::networking::network_thread::PlayerStateMsg;

    use super::*;

    pub async fn recv_driver(
        id: NetworkId,
        mut incoming: Datagrams,
        to_server: UnboundedSender<(NetworkId, u32, PlayerStateMsg)>,
    ) -> Result<()> {
        let mut prev_tag = 0;
        let mut msg_buf = Vec::new();
        while let Some(datagram) = incoming.next().await {
            let buf = &(&datagram?)[..];
            //receive_bytes(&mut incoming, &mut buf, 512).await?;   
            
            let mut reader = BitReader::new(buf);
            let mut tag = reader.uint(16) as u16;
            //println!("Received {} bytes @ tag: {tag}", buf.len());

            let latest_input = PlayerStateMsg {
                tag,
                delta_pos: reader.bool().then(|| vec3(
                    decode_velocity(reader.uint(16)),
                    decode_velocity(reader.uint(16)),
                    decode_velocity(reader.uint(16)),
                )),
                delta_yaw_pitch: reader.bool().then(|| vec2(
                    decode_angle_rad(reader.uint(16) as u16),
                    decode_angle_rad(reader.uint(16) as u16),
                )),
            };
            //println!("Got: {latest_input:?}");

            if tag == prev_tag {
                continue;
            }
            let mut packets_lost = tag.wrapping_sub(prev_tag);
            prev_tag = tag;

            msg_buf.clear();
            msg_buf.push(latest_input);

            let mut num_missing = packets_lost;
            while num_missing > 1 && reader.bool() {
                tag = tag.wrapping_sub(1);
                num_missing -= 1;

                msg_buf.push(PlayerStateMsg {
                    tag,
                    delta_pos: reader.bool().then(|| vec3(
                        decode_velocity(reader.uint(16)),
                        decode_velocity(reader.uint(16)),
                        decode_velocity(reader.uint(16)),
                    )),
                    delta_yaw_pitch: reader.bool().then(|| vec2(
                        decode_angle_rad(reader.uint(16) as u16),
                        decode_angle_rad(reader.uint(16) as u16),
                    )),
                });
                //println!("Recovered {tag}: {:?}", msg_buf.last().unwrap());
            }

            for msg in msg_buf.drain(..).rev() {
                let _ = to_server.send((id, packets_lost as u32-1, msg));
                packets_lost = 1;
            }
        }
        Ok(())
    }
}

pub mod entity_state {
    use glam::Vec3;
    use shared::{bits_and_bytes::ByteWriter, protocol::{encode_velocity, encode_angle_rad, wrap_angle}};

    use crate::components::{YawPitch, NetworkId};

    use super::*;

    pub struct EntityStateOut {
        pub player_input_tag: Option<u16>,
        pub packets_lost: u8,
        pub player_pos: Vec3,
        pub player_head_rot: YawPitch,
        pub changes: Vec<(NetworkId, EntityStateMsg)>,
    }

    #[derive(Clone, Copy)]
    pub enum EntityStateMsg {
        EntityAdded {
            position: Vec3,
            head_rotation: YawPitch
        },
        EntityRemoved,
        EntityMoved {
            delta_pos: Vec3,
            delta_head_rotation: YawPitch,
        }
    }

    pub async fn send_driver(
        mut outgoing: SendStream,
        mut messages: UnboundedReceiver<EntityStateOut>,
    ) -> Result<()> {
        //println!("entity_state::send_driver ready");
        let mut send_buf = vec![0u8; 3072];
        let mut prev_input_tag = u16::MAX; // Client has the same "uninitialized" tag
        while let Some(msg) = messages.recv().await {
            let EntityStateOut { 
                player_input_tag, 
                packets_lost,
                player_pos, 
                player_head_rot, 
                changes 
            } = msg;

            let mut writer = ByteWriter::new_for_message(&mut send_buf);
            if let Some(tag) = player_input_tag {
                //println!("Out tag: {tag}");
                if tag == prev_input_tag {
                    panic!("Some(tag) = prev_tag");
                }

                writer.write_u16(tag);
                writer.write_u8(packets_lost);
                writer.write_f32(player_pos.x);
                writer.write_f32(player_pos.y);
                writer.write_f32(player_pos.z);

                writer.write_f32(player_head_rot.x);
                writer.write_f32(player_head_rot.y);
                prev_input_tag = tag;
            } else {
                writer.write_u16(prev_input_tag);
                // Client will know there is no associated data because this tag was previously processed
            }
            let base_length = writer.bytes_written();

            for (id, event) in changes {
                match event {
                    EntityStateMsg::EntityAdded { position, head_rotation } => {
                        // TODO, this way of writing the IDs
                        // - consumes more bandwidth than necessary
                        // - limits max entity count in the ENTIRE world to 2^(15-2)=8192
                        writer.write_varint15((id.raw() << 2) | 0b00);
                        writer.write_f32(position.x);
                        writer.write_f32(position.y);
                        writer.write_f32(position.z);
                        writer.write_f32(head_rotation.x);
                        writer.write_f32(head_rotation.y);
                    },
                    EntityStateMsg::EntityRemoved => {
                        writer.write_varint15((id.raw() << 2) | 0b10);
                    },
                    EntityStateMsg::EntityMoved { delta_pos, delta_head_rotation } => {
                        writer.write_varint15(((id.raw()) << 1) | 0b1);
                        writer.write_u16(encode_velocity(delta_pos.x) as u16);
                        writer.write_u16(encode_velocity(delta_pos.y) as u16);
                        writer.write_u16(encode_velocity(delta_pos.z) as u16);
                        writer.write_u16(encode_angle_rad(wrap_angle(delta_head_rotation.x)));
                        writer.write_u16(encode_angle_rad(wrap_angle(delta_head_rotation.y)));
                    },
                }
            }
            writer.write_message_len();

            if writer.bytes_written() > base_length {
                outgoing.write_all(writer.bytes()).await?;
            }
        }
        Ok(())
    }
}