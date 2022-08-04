use erupt::vk;
use glam::{Vec3, Vec2};
use vkcore::{VkContext, BufferAllocation, UsageFlags, Buffer};

use crate::{
    game_builder::GameBuilder,
    chat, Vertex, player,
};

pub fn init(app: &mut GameBuilder) {
    player::init(app);
    chat::init(app);
}

fn create_cube_mesh(center: Vec3, dims: Vec3, vk: &VkContext) -> Buffer {
    let mut corners = [
        // Initially pos x
        Vec3::new(0.5, -0.5, -0.5),
        Vec3::new(0.5, -0.5,  0.5),
        Vec3::new(0.5,  0.5, -0.5),
        Vec3::new(0.5,  0.5,  0.5),
    ];

    let mut buf = [
        Vertex { pos: Vec3::new(0.5, -0.5,  0.5), col: Vec3::ONE, uv: Vec2::ZERO },
        Vertex { pos: Vec3::new(0.5, -0.5, -0.5), col: Vec3::ONE, uv: Vec2::ZERO },
        Vertex { pos: Vec3::new(0.5,  0.5,  0.5), col: Vec3::ONE, uv: Vec2::ZERO },
        Vertex { pos: Vec3::new(0.5,  0.5,  0.5), col: Vec3::ONE, uv: Vec2::ZERO },
        Vertex { pos: Vec3::new(0.5, -0.5, -0.5), col: Vec3::ONE, uv: Vec2::ZERO },
        Vertex { pos: Vec3::new(0.5,  0.5, -0.5), col: Vec3::ONE, uv: Vec2::ZERO },

        Vertex { pos: Vec3::new(-0.5, -0.5, -0.5), col: Vec3::ONE/2.0, uv: Vec2::ZERO },
        Vertex { pos: Vec3::new(-0.5, -0.5,  0.5), col: Vec3::ONE/2.0, uv: Vec2::ZERO },
        Vertex { pos: Vec3::new(-0.5,  0.5, -0.5), col: Vec3::ONE/2.0, uv: Vec2::ZERO },
        Vertex { pos: Vec3::new(-0.5,  0.5, -0.5), col: Vec3::ONE/2.0, uv: Vec2::ZERO },
        Vertex { pos: Vec3::new(-0.5, -0.5,  0.5), col: Vec3::ONE/2.0, uv: Vec2::ZERO },
        Vertex { pos: Vec3::new(-0.5,  0.5,  0.5), col: Vec3::ONE/2.0, uv: Vec2::ZERO },

        Vertex { pos: Vec3::new(  0.5, -0.5, 0.5), col: Vec3::ONE, uv: Vec2::ZERO },
        Vertex { pos: Vec3::new(  0.5,  0.5, 0.5), col: Vec3::ONE, uv: Vec2::ZERO },
        Vertex { pos: Vec3::new( -0.5, -0.5, 0.5), col: Vec3::ONE, uv: Vec2::ZERO },
        Vertex { pos: Vec3::new( -0.5, -0.5, 0.5), col: Vec3::ONE, uv: Vec2::ZERO },
        Vertex { pos: Vec3::new(  0.5,  0.5, 0.5), col: Vec3::ONE, uv: Vec2::ZERO },
        Vertex { pos: Vec3::new( -0.5,  0.5, 0.5), col: Vec3::ONE, uv: Vec2::ZERO },

        Vertex { pos: Vec3::new( -0.5, -0.5, -0.5), col: Vec3::ONE, uv: Vec2::ZERO },
        Vertex { pos: Vec3::new( -0.5,  0.5, -0.5), col: Vec3::ONE, uv: Vec2::ZERO },
        Vertex { pos: Vec3::new(  0.5, -0.5, -0.5), col: Vec3::ONE, uv: Vec2::ZERO },
        Vertex { pos: Vec3::new(  0.5, -0.5, -0.5), col: Vec3::ONE, uv: Vec2::ZERO },
        Vertex { pos: Vec3::new( -0.5,  0.5, -0.5), col: Vec3::ONE, uv: Vec2::ZERO },
        Vertex { pos: Vec3::new(  0.5,  0.5, -0.5), col: Vec3::ONE, uv: Vec2::ZERO },

        Vertex { pos: Vec3::new(  0.5, 0.5,  0.5), col: Vec3::ONE, uv: Vec2::ZERO },
        Vertex { pos: Vec3::new(  0.5, 0.5, -0.5), col: Vec3::ONE, uv: Vec2::ZERO },
        Vertex { pos: Vec3::new( -0.5, 0.5,  0.5), col: Vec3::ONE, uv: Vec2::ZERO },
        Vertex { pos: Vec3::new( -0.5, 0.5,  0.5), col: Vec3::ONE, uv: Vec2::ZERO },
        Vertex { pos: Vec3::new(  0.5, 0.5, -0.5), col: Vec3::ONE, uv: Vec2::ZERO },
        Vertex { pos: Vec3::new( -0.5, 0.5, -0.5), col: Vec3::ONE, uv: Vec2::ZERO },

        Vertex { pos: Vec3::new( -0.5, -0.5,  0.5), col: Vec3::ONE, uv: Vec2::ZERO },
        Vertex { pos: Vec3::new( -0.5, -0.5, -0.5), col: Vec3::ONE, uv: Vec2::ZERO },
        Vertex { pos: Vec3::new(  0.5, -0.5,  0.5), col: Vec3::ONE, uv: Vec2::ZERO },
        Vertex { pos: Vec3::new(  0.5, -0.5,  0.5), col: Vec3::ONE, uv: Vec2::ZERO },
        Vertex { pos: Vec3::new( -0.5, -0.5, -0.5), col: Vec3::ONE, uv: Vec2::ZERO },
        Vertex { pos: Vec3::new(  0.5, -0.5, -0.5), col: Vec3::ONE, uv: Vec2::ZERO },
    ];

    let mut vbo = vk.allocator.allocate_buffer(&vk.device, &BufferAllocation {
        size: 36 * std::mem::size_of::<Vertex>(),
        usage: UsageFlags::FAST_DEVICE_ACCESS,
        vk_usage: vk::BufferUsageFlags::VERTEX_BUFFER,
    }).unwrap();
    vk.uploader.upload_to_buffer(&vk.device, &buf, &mut vbo, 0).unwrap();

    vbo
}