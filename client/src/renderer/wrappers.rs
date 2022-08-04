use erupt::vk;
use vkcore::Buffer;

pub struct IndexBuffer {
    pub buffer: Buffer,
    pub index_type: vk::IndexType,
}

pub struct VertexBuffer {
    pub buffer: Buffer,
    pub vertex_count: u32,
}