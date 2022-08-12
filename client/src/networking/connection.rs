use quinn::{RecvStream, SendStream};

use shared::bits_and_bytes::ByteReader;
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
    use shared::bits_and_bytes::ByteWriter;

    use super::*;

    pub async fn recv_driver(
        mut incoming: RecvStream,
        to_main: Sender<S2C>,
    ) -> anyhow::Result<()> {
        let mut buf = Vec::new();
        loop {
            let mut stream = receive_bytes(&mut incoming, &mut buf).await?;

            let msg = stream.read_str(stream.bytes_remaining());
            let _ = to_main.send(S2C::Chat(msg.to_shared_str())).await;
        }
    }

    pub async fn send_driver(
        mut outgoing: SendStream,
        mut messages: UnboundedReceiver<SharedStr>,
    ) -> anyhow::Result<()> {
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

    use shared::{
        bits_and_bytes::ByteWriter,
        protocol::{decode_angle_rad, decode_velocity},
    };

    use super::*;

    pub async fn recv_driver(
        mut incoming: RecvStream,
        to_main: Sender<S2C>,
    ) -> anyhow::Result<()> {
        let mut recv_buf = Vec::new();
        let mut send_buf = Vec::new();
        loop {
            let mut stream = receive_bytes(&mut incoming, &mut recv_buf).await?;

            let entries = stream.bytes_remaining() / 12;
            
            send_buf.resize(entries * 58, 0);
            let mut writer = ByteWriter::new(&mut send_buf);

            for _ in 0..entries {
                writer.write_u16(stream.read_u16());
                writer.write_f32(decode_velocity(stream.read_u16() as u32));
                writer.write_f32(decode_velocity(stream.read_u16() as u32));
                writer.write_f32(decode_velocity(stream.read_u16() as u32));
                writer.write_f32(decode_angle_rad(stream.read_u16()));
                writer.write_f32(decode_angle_rad(stream.read_u16()));
            }

            if writer.bytes_written() > 0 {
                let _ = to_main.send(S2C::EntityState(writer.bytes().into())).await;
            }
        }
    }
}

pub(super) mod player_state {
    use super::*;

    pub async fn send_driver(
        mut outgoing: SendStream,
        mut messages: UnboundedReceiver<Box<[u8]>>,
    ) -> anyhow::Result<()> {
        while let Some(message) = messages.recv().await {
            outgoing.write_all(&message).await?;
        }
        Ok(())
    }
}
