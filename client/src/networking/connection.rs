use quinn::{RecvStream, SendStream};

use shared::bits_and_bytes::ByteReader;
use tokio::sync::mpsc::{UnboundedSender, UnboundedReceiver};

enum MessageStatus {
    Consumed(usize),
    NotEnoughData,
    Error,
}

async fn generic_recv_driver<F: FnMut(ByteReader) -> MessageStatus>(
    mut incoming: RecvStream,
    buffer_size: usize,
    mut callback: F,
) -> anyhow::Result<()> {
    let mut recv_buffer = Vec::new();
    recv_buffer.resize(buffer_size, 0);

    let mut offset = 0;

    loop {
        let res = incoming.read(&mut recv_buffer[offset..]).await;
        let bytes_received = match res {
            Ok(Some(num_bytes)) => num_bytes,
            Ok(None) => continue,
            Err(e) => {
                println!("Error on incoming.read(): {}", e);
                break;
            }
        };

        let total_num_bytes = offset + bytes_received;

        //println!("Received {} bytes; total: {total_num_bytes}", bytes_received);

        match (callback)(ByteReader::new(&recv_buffer[..total_num_bytes])) {
            MessageStatus::Consumed(num_bytes) => {
                if num_bytes < total_num_bytes {
                    let remaining = total_num_bytes - num_bytes;
                    println!("Moving {} bytes", remaining);
                    recv_buffer
                        .as_mut_slice()
                        .copy_within(num_bytes..total_num_bytes, 0);
                    offset = remaining;
                } else {
                    offset = 0;
                }
            }
            MessageStatus::Error => return Ok(()), // something wrong in callback, exit here
            MessageStatus::NotEnoughData => {}
        }
    }
    Ok(())
}

pub(super) mod chat {
    use flexstr::SharedStr;
    use shared::bits_and_bytes::ByteWriter;

    use crate::chat::Chat;

    use super::*;

    pub async fn recv_driver(incoming: RecvStream) -> anyhow::Result<()> {
        generic_recv_driver(incoming, 512, move |mut stream| {
            if stream.bytes_remaining() < 2 {
                return MessageStatus::NotEnoughData;
            }

            let length = stream.read_u16() as usize;
            println!("Received message with length {}", length);
            if stream.bytes_remaining() < length {
                return MessageStatus::NotEnoughData;
            }

            let message = stream.read_str(length);
            Chat::write(message.to_owned(), 0xFF_FF_FF_FF);

            MessageStatus::Consumed(stream.bytes_read())
        })
        .await
    }

    pub async fn send_driver(
        mut outgoing: SendStream,
        mut messages: UnboundedReceiver<SharedStr>,
    ) -> anyhow::Result<()> {
        let mut buf = [0u8; 512];
        while let Some(message) = messages.recv().await {
            println!("Sending '{message}'");
            let mut writer = ByteWriter::new(&mut buf);
            writer.write_u16(message.len() as u16);
            writer.write(message.as_bytes());
            let length = writer.bytes_written();
            outgoing.write_all(&buf[..length]).await?;
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

    use shared::{bits_and_bytes::{BitReader, ByteWriter}, protocol::NetworkId};

    use super::*;

    pub async fn recv_driver(incoming: RecvStream, to_main: UnboundedSender<Vec<u8>>) -> anyhow::Result<()> {
        use MessageStatus::*;

        let mut buf = Vec::new();
        buf.resize(2048, 0);

        generic_recv_driver(incoming, 8192, move |mut stream| {
            let mut reader = BitReader::new(stream.bytes());
            if stream.bytes_remaining() < 3 {
                return NotEnoughData;
            }

            let len = reader.uint(11) as usize + 3;
            //println!("Received {len} bytes of entity state data. Buffered: {}", stream.bytes_remaining());
            if stream.bytes_remaining() < len {
                return NotEnoughData;
            }

            let word = ByteReader::new(stream.bytes()).read_u64();
            let word2 = BitReader::new(stream.bytes()).uint(32);

            let mut writer = ByteWriter::new(&mut buf);

            let entries = reader.uint(13);
            //println!("Entries: {entries}, word: {word} vs {word2}");
            for _ in 0..entries {
                writer.write_u16(reader.uint(16) as u16);
                let x = ((reader.uint(8) as i32 - 128) as f32 / 500.0);
                let y = ((reader.uint(8) as i32 - 128) as f32 / 500.0);
                let z = ((reader.uint(8) as i32 - 128) as f32 / 500.0);

                //println!("Delta: {x:.8}, {y:.8}, {z:.8}");

                writer.write_f32(x);
                writer.write_f32(y);
                writer.write_f32(z);
            }

            let len = writer.bytes_written();
            to_main.send((&buf[..len]).to_vec()).unwrap();
            
            Consumed(len)
            /* if stream.bytes_remaining() < 2 {
                return MessageStatus::NotEnoughData;
            }

            let length = stream.read_u16() as usize;
            if stream.bytes_remaining() < length {
                return MessageStatus::NotEnoughData;
            }

            let mut data = Vec::new(); data.resize(length, 0u8);
            stream.read(&mut data[..]);

            if to_main.send(data).is_err() {
                MessageStatus::Error
            } else {
                MessageStatus::Consumed(stream.bytes_read())
            } */
        })
        .await
    }
}

pub(super) mod player_state {
    use super::*;

    pub async fn send_driver(
        mut outgoing: SendStream,
        mut messages: UnboundedReceiver<Vec<u8>>,
    ) -> anyhow::Result<()> {
        while let Some(message) = messages.recv().await {
            outgoing.write_all(&message).await?;
        }
        Ok(())
    }
}

/* pub async fn listen_to_server(
    mut in_stream: RecvStream,
    to_main: UnboundedSender<Vec<u8>>,
) -> Result<()> {
    let mut recv_buffer = Vec::new();
    recv_buffer.resize(2048, 0u8); // 2KB per connection = 500 per MiB...

    let mut offset = 0usize;

    while let Some(bytes_received) = in_stream.read(&mut recv_buffer[offset..]).await? {
        let total_bytes = offset + bytes_received;

        let mut reader = BinaryReader::new(&recv_buffer[..total_bytes]);
        let mut num_bytes = 0;
        let mut num_messages = 0;
        loop {
            reader.mark_start();
            let header = match s2c::read_header(&mut reader) {
                Ok(header) => header,
                Err(MessageError::NotEnoughData) => break,
                Err(MessageError::Malformed) => {
                    todo!("Kick player on malformed Messages")
                }
            };

            if header.size_bytes < reader.bytes_remaining() {
                break;
            }

            num_bytes += (reader.bytes_read() + header.size_bytes) as usize;
            num_messages += 1;
            reader.skip(header.size_bytes);
        }

        if num_bytes == 0 {
            continue;
        }
        //println!("Received {} bytes / {} Messages!", num_bytes, num_Messages);
        to_main.send(recv_buffer[..num_bytes].to_vec())?;

        if num_bytes == total_bytes {
            offset = 0;
        } else {
            let remaining = total_bytes - num_bytes;
            //println!("Moving {} bytes", remaining);
            recv_buffer
                .as_mut_slice()
                .copy_within(num_bytes..total_bytes, 0);
            offset = remaining;
        }
    }
    println!("CONNECTION FINISHED");
    Ok(())
}
 */
