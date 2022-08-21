use quinn::{RecvStream, SendStream};

use shared::bits_and_bytes::{ByteWriter, ByteReader};
use tokio::sync::mpsc::UnboundedReceiver;

use tokio::sync::mpsc::Sender;

use crate::networking::S2C;

pub async fn receive_bytes<'a>(stream: &mut RecvStream, buf: &'a mut Vec<u8>) -> anyhow::Result<ByteReader<'a>> {
    let mut header = [0u8; 2];
    stream.read_exact(&mut header[0..2]).await?;

    let mut length = header[0] as usize;    
    if length > 127 {
        length = length - 128 + ((header[1] as usize) << 7);
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
    use flexstr::{SharedStr, ToSharedStr};
    use super::*;

    pub async fn recv_driver(mut incoming: RecvStream, to_main: Sender<S2C>) -> anyhow::Result<()> {
        let mut buf = Vec::new();
        loop {
            let mut stream = receive_bytes(&mut incoming, &mut buf).await?;

            let msg = stream.read_str(stream.bytes_remaining());
            let _ = to_main.send(S2C::Chat(msg.to_shared_str())).await;
        }
    }

    pub async fn send_driver(mut outgoing: SendStream, mut messages: UnboundedReceiver<SharedStr>) -> anyhow::Result<()> {
        let mut buf = [0u8; 512];
        while let Some(message) = messages.recv().await {
            let mut writer = ByteWriter::new_for_message(&mut buf);
            writer.write(message.as_bytes());
            writer.write_message_len();
            outgoing.write_all(writer.bytes()).await?;
        }
        Ok(())
    }
}

pub(super) mod entity_state {
    /*
    - Once per tick
    - Contains the data for *all* entities
    EntityStatesMessage:
        Length u16
        NumEntries u16 // entry per entity
        FirstEntityID VarInt
        BitsPerIdDelta u8
        Entry:
            EntityIdDelta ? bits
            Contents bitmap: (4 bits now but will probably expand)
                1 << 0: Position changed (absolute)
                1 << 1: Velocity changed (relative)
                1 << 2: Facing changed
                1 << 3: Entity was hurt

            (Optional) position: 3 x FixedPoint_14_9 // 14 bit whole part, 7 bit frac part (1/128)
            (Optional) velocity: 3 x FixedPoint_3_7 // 3 bit whole (-3..3), 7 bit frac part
            (Optional) facing:   2 x u8 (yaw & pitch, 0..360 mapped to 0..255)
        x NumEntries (Sorted ascending by entity id)
    */

    use glam::{vec3, vec2};
    use shared::{
        protocol::{decode_angle_rad, decode_velocity, NetworkId},
    };

    use crate::networking::EntityStateMsg;

    use super::*;

    pub async fn recv_driver(
        mut incoming: RecvStream,
        to_main: Sender<S2C>,
    ) -> anyhow::Result<()> {
        let mut recv_buf = Vec::new();
        let mut send_buf = Vec::new();

        let mut prev_tag = u16::MAX; // Server has the same "uninitialized" tag
        loop {
            send_buf.clear();

            let mut stream = receive_bytes(&mut incoming, &mut recv_buf).await?;
            //println("Got {} bytes", stream.bytes_remaining());
            
            let tag = stream.read_u16();
            if tag != prev_tag {
                //println("> Tag: {tag}, prev tag: {prev_tag}");
                // New info
                send_buf.push(EntityStateMsg::InputValidated { 
                    tag, 
                    packets_lost: stream.read_u8(),
                    server_pos: vec3(
                        stream.read_f32(),
                        stream.read_f32(),
                        stream.read_f32()
                    ), 
                    server_head_rot: vec2(
                        stream.read_f32(),
                        stream.read_f32()
                    )
                });
                prev_tag = tag;
            } else {
                //println("> Same tag");
            }

            while stream.bytes_remaining() > 0 {
                let start = stream.read_varint15();
                match start & 0b11 {
                    0b00 => {
                        //println("> EntityAdded @ {}", start >> 2);
                        send_buf.push(EntityStateMsg::EntityAdded{
                            id: NetworkId::from_raw(start >> 2),
                            position: vec3(stream.read_f32(), stream.read_f32(), stream.read_f32()),
                            head_rotation: vec2(stream.read_f32(), stream.read_f32()),
                        });
                    }
                    0b10 => {
                        //println("> EntityRemoved @ {}", start >> 2);
                        send_buf.push(EntityStateMsg::EntityRemoved {
                            id: NetworkId::from_raw(start >> 2),
                        });
                    }
                    _ => {
                        send_buf.push(EntityStateMsg::EntityMoved { 
                            id: NetworkId::from_raw(start >> 1), 
                            delta_pos: vec3(
                                decode_velocity(stream.read_u16() as u32),
                                decode_velocity(stream.read_u16() as u32),
                                decode_velocity(stream.read_u16() as u32),
                            ), 
                            delta_head_rotation: vec2(
                                decode_angle_rad(stream.read_u16()),
                                decode_angle_rad(stream.read_u16()),
                            )
                        });
                    }
                }
            }

            let _ = to_main.send(S2C::EntityState(send_buf.as_slice().into())).await;
        }
    }
}

pub(super) mod player_state {
    use bytes::Bytes;
    use glam::{Vec3, Vec2};
    use rand::{thread_rng, RngCore};
    use shared::{bits_and_bytes::BitWriter, protocol::{encode_velocity, encode_angle_rad, wrap_angle}};

    use crate::states::game::input_recorder::InputSnapshot;

    use super::*;

    pub async fn send_driver(
        outgoing: quinn::Connection,
        stats_in: Sender<S2C>,
        mut messages: UnboundedReceiver<Box<[InputSnapshot]>>,
    ) -> anyhow::Result<()> {
        let mut buf = [0u8; 260];

        let mut drop_chance = 10;
        let mut dropped = 0;
        let mut total = 0;
        while let Some(message) = messages.recv().await {
            let _ = stats_in.send(S2C::Statistics{ ping: outgoing.rtt().as_millis() as u32 }).await;

            total += 1;
            if thread_rng().next_u32() % drop_chance == 0 {
/*                 if drop_chance != 10 {
                    drop_chance = 2;
                } else {
                    drop_chance += 2;
                }
                dropped += 1;
 */                //print!("Dropping {}; ", message.last().unwrap().tag);
                continue;
            } else {
                //print!("Letting {} through; ", message.last().unwrap().tag);
            }


            //println!("Dropped {dropped}/{total} ({:.2}%)", dropped as f32 / total as f32 * 100.0);

            let latest = message.last().unwrap();
            
            let mut writer = BitWriter::new(&mut buf);
            writer.uint(latest.tag as u32, 16);

            if writer.bool(latest.delta_position != Vec3::ZERO) {
                writer.uint(encode_velocity(latest.delta_position.x), 16);
                writer.uint(encode_velocity(latest.delta_position.y), 16);
                writer.uint(encode_velocity(latest.delta_position.z), 16);
                //println("Writing velocity: {}", latest.delta_position);
            }
            if writer.bool(latest.delta_rotation != Vec2::ZERO) {
                writer.uint(encode_angle_rad(wrap_angle(latest.delta_rotation.x)) as u32, 16);
                writer.uint(encode_angle_rad(wrap_angle(latest.delta_rotation.y)) as u32, 16);
                //println("Writing rotation: {}", latest.delta_rotation);
            }

            // NOTE reverse order. Latest snapshot is first (above). This is so that 
            // if no previous snapshots are missing, then there is no need to parse all of the
            // snapshots just to get to the needed (latest) snapshot.

            for snapshot in (&message[..message.len()-1]).iter().rev().take(6) {
                writer.bool(true); // has next

                let &InputSnapshot {
                    tag: _,
                    delta_position,
                    delta_rotation,
                    ..
                } = snapshot;

                ////println("Delta for tick {}: {:.8}, {:.8}, {:.8}, pos {:.8}, {:.8}, {:.8}", self.tick, velocity.x, velocity.y, velocity.z, origin.x, origin.y, origin.z);
                if writer.bool(delta_position.length_squared() != 0.0) {
                    ////println("Moved {} units in nw frame", velocity.length());
                    writer.uint(encode_velocity(delta_position.x), 16);
                    writer.uint(encode_velocity(delta_position.y), 16);
                    writer.uint(encode_velocity(delta_position.z), 16);
                }
                if writer.bool(delta_rotation.length_squared() != 0.0) {
                    ////println("Rot delta for tick {}: {:.8}, {:.8}, rot: {:.8}, {:.8}", self.tick, yaw.to_degrees(), pitch.to_degrees(), origin.x.to_degrees(), origin.y.to_degrees());
                    writer.uint(encode_angle_rad(wrap_angle(delta_rotation.x)) as u32, 16);
                    writer.uint(encode_angle_rad(wrap_angle(delta_rotation.y)) as u32, 16);
                }
                
            }
            writer.bool(false); // doesn't have next
            writer.flush_partials();
            let len = writer.compute_bytes_written();

            //println!("Sending {} bytes @ tag {}", len, latest.tag);
            outgoing.send_datagram(Bytes::copy_from_slice(&buf[..len]))?;
        }
        Ok(())
    }
}
