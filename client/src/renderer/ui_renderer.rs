use std::ffi::c_void;

use erupt::vk::{self, BufferUsageFlags};
use glam::{IVec2, Vec2, Vec4};
use vkcore::{Buffer, Device, UsageFlags, VkContext};

use crate::camera::Camera;

use super::{
    descriptor_sets::DescriptorSets,
    passes::ui_pass::UiVertex,
    pipelines::Pipelines,
    text_renderer::{Style, TextRenderer, TextColor, ColorRange}, renderer::RenderContext,
};

pub struct UiRenderer {
    vertices: Vec<UiVertex>,
    buffer: Buffer,

    text: TextRenderer,

    num_verts_to_draw: u32,
}

impl UiRenderer {
    pub fn create(
        vk: &mut VkContext,
        descriptors: &DescriptorSets,
        camera: &Camera,
    ) -> anyhow::Result<Self> {
        let buffer = vk.allocator.allocate_buffer(
            &vk.device,
            &vkcore::BufferAllocation {
                size: 8192, // 1024 vertices
                usage: UsageFlags::UPLOAD,
                vk_usage: BufferUsageFlags::VERTEX_BUFFER,
            },
        )?;

        let text = TextRenderer::new(vk, descriptors, camera.proj_view_matrix())?;

        Ok(Self {
            vertices: Vec::with_capacity(1024),
            buffer,
            text,
            num_verts_to_draw: 0,
        })
    }

    pub fn draw_text(&mut self, text: &str, x: u16, y: u16) -> (u16, u16) {
        self.draw_text_styled(text, x, y, Style::default())
    }

    pub fn draw_text_styled(&mut self, text: &str, x: u16, y: u16, style: Style) -> (u16, u16) {
        self.text.draw_2d(text, x, y, style)
    }

    pub fn draw_text_colored(&mut self, text: &str, x: u16, y: u16, color: TextColor) -> (u16, u16) {
        self.text.draw_2d(text, x, y, Style {
            colors: &[ColorRange::new(color, u32::MAX)],
            ..Default::default()
        })
    }

    pub fn draw(&mut self, vertices: &[UiVertex]) {
        self.vertices.extend_from_slice(vertices);
    }

    pub fn draw_rect_xy_wh(&mut self, (x, y): (u16, u16), (w, h): (u16, u16), color: u32) {
        let color = color.to_be();
        self.draw(&[
            UiVertex::color(x, y, color),
            UiVertex::color(x, y + h, color),
            UiVertex::color(x + w, y, color),
            UiVertex::color(x + w, y, color),
            UiVertex::color(x, y + h, color),
            UiVertex::color(x + w, y + h, color),
        ]);
    }

    // vetices: [((x, y), (r, g, b, a))]
    // (0.0, 0.0) is at bottom left
    pub fn draw_colored(&mut self, vertices: &[(IVec2, Vec4)]) {
        self.vertices.reserve(vertices.len());
        for (pos, color) in vertices {
            let color = color.as_uvec4();
            let color = (color.x << 24) | (color.y << 16) | (color.z << 8) | color.w;

            self.vertices.push(UiVertex {
                x: pos.x as u16,
                y: pos.y as u16,
                color_or_uv: color,
            });
        }
    }

    pub fn text(&mut self) -> &mut TextRenderer {
        &mut self.text
    }
}

impl UiRenderer {
    pub fn do_uploads(renderer: &mut UiRenderer, vk: &mut VkContext, frame: usize) -> anyhow::Result<()> {
        if renderer.vertices.is_empty() {
            return Ok(());
        }

        let buffer = &mut renderer.buffer;
        let vertices = &renderer.vertices;

        let buffer_size = vertices.len() * std::mem::size_of::<UiVertex>();
        if buffer.size < buffer_size as u64 {
            println!(
                "[ui_renderer.rs] Buffer size is too small, reallocating! {} -> {} bytes",
                buffer.size,
                vertices.capacity() * std::mem::size_of::<UiVertex>()
            );

            vk.allocator.deallocate_buffer(buffer, &vk.device)?;
            *buffer = vk.allocator.allocate_buffer(
                &vk.device,
                &vkcore::BufferAllocation {
                    size: vertices.capacity() * std::mem::size_of::<UiVertex>(),
                    usage: UsageFlags::UPLOAD,
                    vk_usage: BufferUsageFlags::VERTEX_BUFFER,
                },
            )?;
        }

        vk.uploader
            .upload_to_buffer(&vk.device, vertices, buffer, 0)?;

        renderer.num_verts_to_draw = renderer.vertices.len() as _;
        renderer.vertices.clear();

        TextRenderer::do_uploads(&mut renderer.text, vk, frame)
    }

    pub fn render(
        renderer: &mut UiRenderer,
        device: &Device,
        ctx: &RenderContext,
        pipelines: &Pipelines,
        descriptors: &DescriptorSets,
        wnd_size: Vec2
    ) {
        let commands = ctx.commands;
        unsafe {
            device.cmd_bind_pipeline(
                commands,
                vk::PipelineBindPoint::GRAPHICS,
                pipelines.ui.shapes.handle,
            );
            // `2.0 / ..` because coordinate space is from -1 to 1 (so 2 units)
            let pv = 2.0 / wnd_size;
            let pvm_ptr = &pv as *const Vec2 as *const c_void;
            device.cmd_push_constants(
                commands,
                pipelines.ui.shapes.layout,
                vk::ShaderStageFlags::VERTEX,
                0,
                std::mem::size_of::<Vec2>() as u32,
                pvm_ptr,
            );

            device.cmd_bind_vertex_buffers(commands, 0, &[renderer.buffer.handle], &[0]);
            device.cmd_draw(commands, renderer.num_verts_to_draw, 1, 0, 0);
        }

        renderer.num_verts_to_draw = 0;

        TextRenderer::render(&mut renderer.text, device, pipelines, descriptors, ctx);
    }

    pub fn handle_window_resize(renderer: &mut UiRenderer, vk: &mut VkContext) {
        TextRenderer::handle_window_resize(&mut renderer.text, vk);
    }
}

impl UiRenderer {
    pub fn destroy_self(&mut self, vk: &mut VkContext) -> anyhow::Result<()> {
        vk.allocator
            .deallocate_buffer(&mut self.buffer, &vk.device)?;
        self.text.destroy_self(vk)?;
        Ok(())
    }
}
