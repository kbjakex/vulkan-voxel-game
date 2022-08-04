use crate::bits_and_bytes::ByteReader;

pub mod login {
    use crate::{protocol::{MessageError, PROTOCOL_MAGIC, PROTOCOL_VERSION}, bits_and_bytes::ByteWriter};

    use super::*;

    pub struct LoginMessage<'a> {
        pub username: &'a str,
    }

    impl<'a> LoginMessage<'a> {
        //#[cfg(feature = "server")]
        pub fn parse(bytes: &'a [u8]) -> Result<LoginMessage<'a>, MessageError> {
            let mut stream = ByteReader::new(bytes);
            if stream.bytes_remaining() < 6 {
                return Err(MessageError::NotEnoughData);
            }

            if stream.read_u16() != PROTOCOL_MAGIC || stream.read_u16() != PROTOCOL_VERSION {
                // 4/6
                return Err(MessageError::Malformed);
            }

            let name_len = stream.read_u16() as usize; // 6/6
            if stream.bytes_remaining() < name_len {
                return Err(MessageError::NotEnoughData);
            }

            Ok(LoginMessage {
                username: stream.read_str(name_len),
            })
        }

        //#[cfg(feature = "client")]
        pub fn write(&self, stream: &mut ByteWriter) {
            assert!(stream.space_remaining() >= 2 + 2 + 2 + self.username.len());

            stream.write_u16(PROTOCOL_MAGIC);
            stream.write_u16(PROTOCOL_VERSION);
            stream.write_u16(self.username.len() as u16);
            stream.write(self.username.as_bytes());
        }
    }
}
