pub mod login {
    use glam::Vec3;

    use crate::{protocol::{PROTOCOL_MAGIC, PROTOCOL_VERSION, NetworkId, MessageError}, bits_and_bytes::{ByteReader, ByteWriter}};

    pub struct LoginResponse {
        pub network_id: NetworkId,
        pub position: Vec3,
        pub world_seed: u64,
    }

    impl LoginResponse {
        pub fn parse(bytes: &[u8]) -> Result<LoginResponse, MessageError> {
            let mut stream = ByteReader::new(bytes);
            if stream.bytes_remaining() < 26 {
                return Err(MessageError::NotEnoughData);
            }

            if stream.read_u16() != PROTOCOL_MAGIC || stream.read_u16() != PROTOCOL_VERSION { // 2/26, 4/26
                return Err(MessageError::Malformed);
            }

            let network_id = NetworkId::from_raw(stream.read_u16()); // 6/26
            let position = Vec3::new(
                stream.read_f32(), // 10/26
                stream.read_f32(), // 14/26
                stream.read_f32() // 18/26
            );
            let world_seed = stream.read_u64(); // 26/26

            Ok(LoginResponse { network_id, position, world_seed })
        }

        //#[cfg(feature = "client")]
        pub fn write(&self, stream: &mut ByteWriter) {
            assert!(stream.space_remaining() >= 18);

            stream.write_u16(PROTOCOL_MAGIC);
            stream.write_u16(PROTOCOL_VERSION);
            stream.write_u16(self.network_id.raw());
            stream.write_f32(self.position.x);
            stream.write_f32(self.position.y);
            stream.write_f32(self.position.z);
            stream.write_u64(self.world_seed);
        }
    }
}
