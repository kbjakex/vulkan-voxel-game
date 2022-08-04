/* use bin_io::writer::BinaryWriter;
use quinn::{SendStream, RecvStream};
use shared::protocol::{c2s, s2c};

use anyhow::{Result, bail};

pub async fn do_login(send_stream: &mut SendStream, recv_stream: &mut RecvStream) -> Result<()> {
    let mut buf = [0u8; 64];

    let message = c2s::login::LoginMessage {
        username: "jetp250"
    };

    let mut writer = BinaryWriter::new(&mut buf);
    message.write(&mut writer);
    let size = writer.bytes_written() as usize;

    println!("Sending {} bytes", size);

    if let Err(e) = send_stream.write_all(&buf[0..size]).await {
        bail!("Failed to send login Message: {}", e);
    }

    let num_bytes = match recv_stream.read(&mut buf).await {
        Ok(Some(bytes)) => bytes,
        Ok(None) => { bail!("Error receiving login response; stream was finished?"); },
        Err(e) => { bail!("Error receiving login response: {}", e); }
    };

    println!("Received login response! Length: {}", num_bytes);

    match s2c::login::LoginResponse::parse(&buf[..num_bytes]) {
        Ok(message) => message,
        Err(message_error) => bail!("Invalid login response: {:?}", message_error),
    };

    println!("Login response received!");

    Ok(())
} */