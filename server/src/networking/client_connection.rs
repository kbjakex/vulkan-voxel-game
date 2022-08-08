use quinn::{RecvStream, SendStream};
use shared::bits_and_bytes::ByteReader;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

use anyhow::{bail, Result};

enum MessageStatus {
    Consumed(usize),
    NotEnoughData,
    Malformed,
    Error,
}

async fn generic_recv_driver<F: FnMut(ByteReader) -> MessageStatus>(
    mut incoming: RecvStream,
    buffer_size: usize,
    mut callback: F,
) -> Result<()> {
    let mut recv_buffer = Vec::new();
    recv_buffer.resize(buffer_size, 0);

    let mut offset = 0;

    while let Some(bytes_received) = incoming.read(&mut recv_buffer[offset..]).await? {
        let total_num_bytes = offset + bytes_received;

        let mut start = 0;
        while start < total_num_bytes {
            match (callback)(ByteReader::new(&recv_buffer[start..total_num_bytes])) {
                MessageStatus::Consumed(num_bytes) => {
                    start += num_bytes;
                }
                MessageStatus::Malformed => bail!("Malformed packet"), // Not having any of that
                MessageStatus::Error => return Ok(()), // something wrong in callback, exit here
                MessageStatus::NotEnoughData => break
            }
        }
        if start < total_num_bytes {
            let remaining = total_num_bytes - start;
            println!("Moving {} bytes", remaining);
            recv_buffer
                .as_mut_slice()
                .copy_within(start..total_num_bytes, 0);
            offset = remaining;
        } else {
            offset = 0;
        }
    }
    Ok(())
}

pub(super) mod chat {
    use flexstr::{SharedStr, ToSharedStr};
    use shared::{protocol::NetworkId, bits_and_bytes::ByteWriter};

    use super::*;

    pub async fn recv_driver(
        incoming: RecvStream,
        username: SharedStr,
        id: NetworkId,
        to_server: UnboundedSender<(NetworkId, SharedStr)>,
    ) -> Result<()> {
        use MessageStatus::*;

        println!("chat::recv_driver ready");
        generic_recv_driver(incoming, 512, move |mut stream| {
            if stream.bytes_remaining() < 2 {
                return NotEnoughData;
            }

            let length = stream.read_u16();
            if stream.bytes_remaining() < length as _ {
                return NotEnoughData;
            }

            let message = username.clone() + ": " + stream.read_str(length as _);
            println!("'{}' (length {})", message, message.len());
            if let Err(e) = to_server.send((id, message)) {
                println!("Error broadcasting chat message: {}", e);
                return Error;
            }

            Consumed(stream.bytes_read() as _)
        })
        .await
    }

    pub async fn send_driver(
        mut outgoing: SendStream,
        mut messages: UnboundedReceiver<SharedStr>,
    ) -> Result<()> {
        println!("chat::send_driver ready");
        let mut buf = [0u8; 512];
        while let Some(message) = messages.recv().await {
            let mut writer = ByteWriter::new(&mut buf);
            writer.write_u16(message.len() as u16);
            writer.write(message.as_bytes());
            let length = writer.bytes_written() as usize;

            println!("Sending '{}' (length {})", message, message.len());
            outgoing.write_all(&buf[..length]).await?;
            println!("Sent.");
        }
        Ok(())
    }
}

pub(super) mod player_state {
    use glam::Vec3;
    use shared::{bits_and_bytes::BitReader, protocol::NetworkId};

    use crate::networking::network_thread::PlayerStateMsg;

    use super::*;

    pub async fn recv_driver(
        id: NetworkId,
        incoming: RecvStream,
        to_server: UnboundedSender<(NetworkId, PlayerStateMsg)>,
    ) -> Result<()> {
        println!("player_state::recv_driver ready");
        use MessageStatus::*;
        generic_recv_driver(incoming, 512, move |stream| {
            let mut reader = BitReader::new(stream.bytes());

            if reader.bool() {
                if stream.bytes_remaining() < 4 {
                    return NotEnoughData;
                }

                let dx = (reader.uint(8) as i32 - 128) as f32 / 500.0;
                let dy = (reader.uint(8) as i32 - 128) as f32 / 500.0;
                let dz = (reader.uint(8) as i32 - 128) as f32 / 500.0;

                let _ = to_server.send((id, PlayerStateMsg {
                    delta_pos: Some(Vec3::new(dx, dy, dz)),
                }));

                return Consumed(4);
            }

            Consumed(1)
        })
        .await
    }
}

pub(super) mod entity_state {
    use super::*;

    pub async fn send_driver(
        mut outgoing: SendStream,
        mut messages: UnboundedReceiver<Vec<u8>>,
    ) -> Result<()> {
        println!("entity_state::send_driver ready");
        while let Some(message) = messages.recv().await {
            outgoing.write_all(&message).await?;
        }
        Ok(())
    }
}

/* pub async fn listen_to_client(
    mut in_stream: RecvStream,
    to_server: UnboundedSender<MetaMessage>,
) -> Result<()> {
    let mut recv_buffer = Vec::new();
    recv_buffer.resize(2048, 0u8); // 2KB per connection = 500 per MiB...

    let mut offset = 0usize;

    /* while let Some(bytes_received) = in_stream.read(&mut recv_buffer[offset..]).await? {
        let total_bytes = offset + bytes_received;

        let mut reader = BinaryReader::new(&recv_buffer[..total_bytes]);
        let mut num_bytes = 0;
        let mut num_Messages = 0;
        loop {
            reader.mark_start();
            let header = match c2s::read_header(&mut reader) {
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
            num_Messages += 1;
            reader.skip(header.size_bytes);
        }

        if num_bytes == 0 {
            continue;
        }
        println!("Received {} bytes / {} Messages!", num_bytes, num_Messages);
        to_server.send(MetaMessage::Message(recv_buffer[..num_bytes].to_vec()))?;

        if num_bytes == total_bytes {
            offset = 0;
        } else {
            let remaining = total_bytes - num_bytes;
            println!("Moving {} bytes", remaining);
            recv_buffer
                .as_mut_slice()
                .copy_within(num_bytes..total_bytes, 0);
            offset = remaining;
        }
    } */
    Ok(())
} */
