use crate::assets;

use bytemuck::{Pod, Zeroable};
use erupt::vk;

use anyhow::Result;
use glam::Mat4;
use smallvec::SmallVec;
use vkcore::{
    Buffer, BufferAllocation, UsageFlags,
    VkContext, Device,
};

use super::{
    descriptor_sets::DescriptorSets,
    renderer::{FRAMES_IN_FLIGHT, RenderContext}, pipelines::Pipelines,
};

const DEFAULT_TEXT_COLOR: TextColor = TextColor::from_rgba(0xFF, 0xFF, 0xFF, 0xFF);

#[derive(Clone, Copy)]
pub enum Align {
    Left,
    Center,
    Right,
}

impl Default for Align {
    fn default() -> Self {
        Align::Left
    }
}

#[derive(Clone, Copy)]
pub struct Style<'a> {
    pub align: Align,
    pub italic: bool,
    pub max_line_width_px: u32, // starting from text x, not x = 0
    pub colors: &'a [ColorRange],
}

impl<'a> Default for Style<'a> {
    fn default() -> Self {
        Self {
            align: Align::Left,
            italic: false,
            max_line_width_px: u32::MAX,
            colors: &[],
        }
    }
}

#[derive(Clone, Copy)]
pub struct TextColor(u32);

impl TextColor {
    pub const fn from_rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self(r8g8b8a8_to_r6g6b6a3(r, g, b, a))
    }

    pub const fn from_rgba32(rgba: u32) -> Self {
        Self(u32_r8g8b8a8_to_r6g6b6a3(rgba))
    }
}

impl Default for TextColor {
    fn default() -> Self {
        Self::from_rgba(255, 255, 255, 255)
    }
}

#[derive(Clone, Copy)]
pub struct ColorRange(TextColor, /* n. glyphs */ u32);

impl Default for ColorRange {
    fn default() -> Self {
        Self::default_for(u32::MAX)
    }
}

impl ColorRange {
    pub const fn default_for(num_glyphs: u32) -> Self {
        Self(DEFAULT_TEXT_COLOR, num_glyphs)
    }

    pub const fn new(color: TextColor, n: u32) -> Self {
        Self(color, n)
    }

    pub const fn from_rgba_n(r: u8, g: u8, b: u8, a: u8, n_glyphs: u32) -> Self {
        Self(TextColor::from_rgba(r, g, b, a), n_glyphs)
    }

    pub const fn from_rgba32_n(rgba: u32, n_glyphs: u32) -> Self {
        Self(TextColor::from_rgba32(rgba), n_glyphs)
    }
}

#[derive(Default, Copy, Clone, Pod, Zeroable)]
#[repr(C)]
struct GlyphVertex {
    // Documented in assets/shaders/text.vert
    d1: u32,
    d2: u32,
}

#[derive(Default, Clone, Copy, Pod, Zeroable)]
#[repr(C)]
struct TextTransform(Mat4);

const fn u32_r8g8b8a8_to_r6g6b6a3(rgba: u32) -> u32 {
    r8g8b8a8_to_r6g6b6a3(
        (rgba >> 24) as u8,
        ((rgba >> 16) & 0xFF) as u8,
        ((rgba >> 8) & 0xFF) as u8,
        (rgba & 0xFF) as u8,
    )
}

const fn r8g8b8a8_to_r6g6b6a3(r: u8, g: u8, b: u8, a: u8) -> u32 {
    const fn clamp_to_range(x: u32) -> u32 {
        if x > 255 {
            255
        } else {
            x
        }
    }

    // Round to nearest color instead of flooring
    let r = clamp_to_range(r as u32 + 2) >> 2;
    let g = clamp_to_range(g as u32 + 2) >> 2;
    let b = clamp_to_range(b as u32 + 2) >> 2;
    let a = clamp_to_range(a as u32 + 4) >> 5;

    (r << 15) | (g << 9) | (b << 3) | a
}

// All units are in pixels.
#[derive(Default, Clone, Copy, Pod, Zeroable)]
#[repr(C)]
struct GlyphData {
    // u32 because `char` isn't Pod
    // Because the glyph table uses perfect hashing,
    // this is here so I can check that the found
    // element really is what I expect it to be
    char: u32,
    // base: 3b, distance from bottom to base. -2..=5, but shifted to 0..=7 here.
    // dims: (3b dim_x << 4) | (4b dim_y)
    base_and_dims: u16, // (base << 7) | (dims) (same as in the shader)
    advance: u8,
    layer: u8, // (3b layer_y << 4) | (4b layer_x)
}

#[derive(Default)]
pub struct PerFrameBuffers {
    glyphs: Buffer,
    transforms: Buffer,
}

pub struct RenderResources {
    pub index_buffer: Buffer,
    pub buffers: [PerFrameBuffers; FRAMES_IN_FLIGHT as usize],
}

impl RenderResources {
    pub fn destroy_self(&mut self, vk: &mut VkContext) -> anyhow::Result<()> {
        for frame in &mut self.buffers {
            vk.allocator
                .deallocate_buffer(&mut frame.glyphs, &vk.device)?;
            vk.allocator
                .deallocate_buffer(&mut frame.transforms, &vk.device)?;
        }
        vk.allocator
            .deallocate_buffer(&mut self.index_buffer, &vk.device)?;
        Ok(())
    }
}

struct Scissors {
    area: vk::Rect2D,
    glyph_count: u32,
}

pub struct TextRenderer {
    rendering: RenderResources,

    text_buffer: Vec<GlyphVertex>, // 1 vertex per glyph
    transform_buffer: Vec<TextTransform>,

    scissors: Vec<Scissors>,
    current_scissor_area: vk::Rect2D,
    current_scissor_start: u32,

    viewport_size: vk::Extent2D,
    proj_view: Mat4,

    glyphs: Box<[GlyphData; 256]>,
}

// Public interface
impl TextRenderer {
    // area in pixels
    pub fn apply_scissors(&mut self, (x, y): (u16, u16), (w, h): (u16, u16)) {
        self.apply_scissors_rect(vk::Rect2D {
            offset: vk::Offset2D { x: x as _, y: y as _ },
            extent: vk::Extent2D { width: w as _, height: h as _ },
        });
    }

    pub fn apply_scissors_rect(&mut self, area: vk::Rect2D) {
        self.end_scissors();

        self.current_scissor_area = area;
        self.current_scissor_start = self.text_buffer.len() as u32;
    }

    pub fn end_scissors(&mut self) {
        // automatic deduplication: if current scissor has glyph count of 0,
        // then current_scissor_start == text_buffer.len(), and it is not added
        if self.current_scissor_start < self.text_buffer.len() as u32 {
            self.scissors.push(Scissors {
                area: self.current_scissor_area,
                glyph_count: self.text_buffer.len() as u32 - self.current_scissor_start,
            });
        }

        self.current_scissor_start = self.text_buffer.len() as u32;
        self.current_scissor_area = vk::Rect2D {
            offset: vk::Offset2D { x: 0, y: 0 },
            extent: self.viewport_size,
        };
    }

    /// (x, y) in in pixels. Returns text width, also in pixels.
    pub fn draw_2d(&mut self, str: &str, x: u16, y: u16, style: Style) -> (u16, u16) {
        if str.is_empty() {
            return (x, y);
        }
        self.draw_2d_chars(str.chars(), x, y, style)
    }

    pub fn draw_2d_chars(
        &mut self,
        str: impl Iterator<Item = char>,
        x: u16,
        y: u16,
        style: Style,
    ) -> (u16, u16) {
        let start_idx = self.text_buffer.len();

        let italic_bit = (style.italic as u32) << 10;

        let mut color_iter = style.colors.iter().copied();
        let mut color = color_iter.next().unwrap_or_default();

        let (mut x, y) = (x as u32, y as u32);

        for char in str {
            let glyph = self.glyphs[char as usize & 0xFF];
            if glyph.char != char as u32 {
                continue;
            }

            while color.1 == 0 {
                color = color_iter.next().unwrap_or_default();
            }
            color.1 -= 1; // glyphs left of this color

            if char != ' ' {
                self.text_buffer.push(GlyphVertex {
                    d1: ((glyph.layer as u32) << 24) | (y << 12) | (x & 0xFFF),
                    d2: (color.0 .0 << 11) | italic_bit | (glyph.base_and_dims as u32),
                });
            }

            x = x.wrapping_add(glyph.advance as u32 * 3);
        }


        let x_offset = match style.align {
            Align::Left => 0,
            Align::Center => x / 2,
            Align::Right => x,
        };
        if x_offset != 0 {
            for vert in &mut self.text_buffer[start_idx..] {
                vert.d1 = vert.d1.wrapping_sub(x_offset); // wrong
            }
        }
        (x as u16, y as u16)
    }

    pub fn compute_glyph_idx_at_pos(&self, str: &str, pos_px: u16) -> usize {
        self.compute_glyph_idx_at_pos_chars(str.chars(), pos_px)
    }

    pub fn compute_glyph_idx_at_pos_chars(&self, str: impl Iterator<Item = char>, pos_px: u16) -> usize {
        let glyphs = &self.glyphs[0..255];
        let pos_px = pos_px as u32;
        let mut x = 0;
        let mut idx = 0;
        for c in str {
            let advance = glyphs[c as usize].advance as u32 * 3;

            if pos_px <= x + advance/2 {
                return idx;
            }
            if pos_px <= x + advance {
                return idx + 1;
            }

            x += advance;
            idx += 1;
        }
        idx
    }

    // scale: width and height of one pixel in the pixel font
    // returns the width in *screen pixels*
    pub fn compute_width(&self, str: &str) -> u16 {
        self.compute_width_chars(str.chars())
    }

    pub fn compute_width_chars(&self, str: impl Iterator<Item = char>) -> u16 {
        let glyphs = &self.glyphs[0..255];
        str.map(|c| glyphs[c as usize & 0xFF].advance as u16)
            .sum::<u16>()
            * 3
    }

    // Returns the byte indices of linebreaks
    pub fn compute_linebreaks(&self, str: &str, max_width_px: u16) -> SmallVec<[u16; 4]> {
        let mut res = SmallVec::new();

        let mut x = 0;
        let mut last_was_space = false;
        let mut split_candidate_idx = 0;
        let mut x_at_split_candidate = 0;
        let mut line_start_idx = 0;

        for (i, c) in str.char_indices() {
            let glyph = self.glyphs[c as usize & 0xFF];
            if glyph.char != c as u32 {
                continue;
            }

            if c == ' ' {
                last_was_space = true;
            } else if last_was_space {
                last_was_space = false;

                split_candidate_idx = i;
                x_at_split_candidate = x;
            }

            x += glyph.advance as u16 * 3;
            if x > max_width_px {
                // Check if there were no spaces in the whole line,
                // and force-split at current glyph if that's the case
                if split_candidate_idx == line_start_idx {
                    split_candidate_idx = i;
                    x_at_split_candidate = x;
                }

                x -= x_at_split_candidate;

                res.push(split_candidate_idx as _);

                line_start_idx = split_candidate_idx;
                x_at_split_candidate = x;
            }
        }

        res.push(str.len() as _);

        res
    }
}

// Internal stuff
impl TextRenderer {
    pub(super) fn new(
        vk: &mut VkContext,
        descriptors: &DescriptorSets,
        proj_view: Mat4,
    ) -> anyhow::Result<Self> {
        init_text_renderer(vk, descriptors, proj_view)
    }

    // Upload data to GPU and prepare data needed to render
    pub fn do_uploads(
        renderer: &mut TextRenderer,
        vk: &mut VkContext,
        frame: usize,
    ) -> anyhow::Result<()> {
        // -1 because first glyph is at index 1, because index 0 is for the scale...
        let num_glyphs = renderer.text_buffer.len() - 1;
        if num_glyphs == 0 {
            return Ok(());
        }

        let size = renderer.viewport_size;

        renderer.end_scissors();
        renderer.current_scissor_start = 1; // Skip the first ""glyph"" aka the scale. Why
        renderer.current_scissor_area = vk::Rect2D {
            // Reset to "no scissor"
            offset: vk::Offset2D { x: 0, y: 0 },
            extent: size,
        };

        let device = &vk.device;
        let uploader = &mut vk.uploader;

        // The absolute most cursed way to pass 'scale' to the shader. Occurrence 2/2.
        renderer.text_buffer[0] = GlyphVertex {
            d1: (2.0 / size.width as f32).to_bits(),
            d2: (2.0 / size.height as f32).to_bits(),
        };

        let vertex_bytes: &[u8] = bytemuck::cast_slice(&renderer.text_buffer);
        uploader.upload_bytes_to_buffer(
            &device,
            vertex_bytes,
            &mut renderer.rendering.buffers[frame].glyphs,
            0,
        )?;
        renderer.text_buffer.drain(1..);

        let transform_bytes: &[u8] = bytemuck::cast_slice(&renderer.transform_buffer);
        uploader.upload_bytes_to_buffer(
            &device,
            transform_bytes,
            &mut renderer.rendering.buffers[frame].transforms,
            0,
        )?;
        renderer.transform_buffer.clear();

        Ok(())
    }

    pub fn render(renderer: &mut TextRenderer, device: &Device, pipelines: &Pipelines, descriptors: &DescriptorSets, ctx: &RenderContext) {
        unsafe {
            device.cmd_bind_pipeline(
                ctx.commands,
                vk::PipelineBindPoint::GRAPHICS,
                pipelines.ui.text.handle,
            );

            device.cmd_bind_descriptor_sets(
                ctx.commands,
                vk::PipelineBindPoint::GRAPHICS,
                pipelines.ui.text.layout,
                0,
                &[
                    descriptors.textures.descriptor_set,
                    descriptors.text_rendering.descriptor_sets[ctx.frame],
                ],
                &[],
            );

            device.cmd_bind_index_buffer(
                ctx.commands,
                renderer.rendering.index_buffer.handle,
                0,
                vk::IndexType::UINT16,
            );

            // Spec mandates that when scissors are dynamic,
            // a scissor *must* be set before drawing.
            // Under Description at https://www.khronos.org/registry/vulkan/specs/1.3-extensions/man/html/VkDynamicState.html

            let mut first_index = 0;
            for scissor in renderer.scissors.drain(..) {
                device.cmd_set_scissor(
                    ctx.commands,
                    0,
                    &[vk::Rect2DBuilder::new()
                        .offset(scissor.area.offset)
                        .extent(scissor.area.extent)],
                );
                device.cmd_draw_indexed(
                    ctx.commands,
                    scissor.glyph_count * 6,
                    1,
                    first_index * 6,
                    0,
                    0,
                );
                first_index += scissor.glyph_count;
            }
        }
    }

    pub fn handle_window_resize(renderer: &mut TextRenderer, vk: &VkContext) {
        renderer.viewport_size = vk.swapchain.surface.extent;
    }

    pub fn on_camera_change(renderer: &mut TextRenderer, proj_view: Mat4) {
        renderer.proj_view = proj_view;
    }

    pub fn destroy_self(&mut self, vk: &mut VkContext) -> anyhow::Result<()> {
        self.rendering.destroy_self(vk)
    }
}

fn init_text_renderer(
    vk: &mut VkContext,
    descriptors: &DescriptorSets,
    proj_view: Mat4,
) -> Result<TextRenderer> {
    //let glyphs = gen_files()?;

    let glyphs_vec = lz4::block::decompress(assets::text::GLYPH_INFO, None)?;
    let mut glyphs = Box::new([GlyphData::default(); 256]);
    glyphs[..].copy_from_slice(bytemuck::cast_slice(&glyphs_vec[..]));

    let ws = vk.swapchain.surface.extent;

    let mut text_buffer = Vec::with_capacity(1024);
    // The absolutely most cursed way to inject the 'scale' variable into the shader. Occurrence 1/2.
    text_buffer.push(GlyphVertex {
        d1: (1.0 / ws.width as f32).to_bits(),
        d2: (1.0 / ws.height as f32).to_bits(),
    });

    let mut transform_buffer = Vec::new();
    transform_buffer.resize(128, TextTransform::default());

    let mut buffers = [(); FRAMES_IN_FLIGHT as usize].map(|_| Default::default());
    for (i, dset) in descriptors
        .text_rendering
        .descriptor_sets
        .iter()
        .copied()
        .enumerate()
    {
        let glyphs = vk.allocator.allocate_buffer(
            &vk.device,
            &BufferAllocation {
                size: 65536, // 65536 / 8 = 8192 glyphs
                usage: UsageFlags::UPLOAD,
                vk_usage: vk::BufferUsageFlags::STORAGE_BUFFER,
            },
        )?;
        let transforms = vk.allocator.allocate_buffer(
            &vk.device,
            &BufferAllocation {
                size: 32768, // 32768 / 16 = 2048 transforms
                usage: UsageFlags::UPLOAD,
                vk_usage: vk::BufferUsageFlags::UNIFORM_BUFFER,
            },
        )?;

        unsafe {
            vk.device.update_descriptor_sets(
                &[
                    vk::WriteDescriptorSetBuilder::new()
                        .dst_binding(0)
                        .dst_set(dset)
                        .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
                        .buffer_info(&[vk::DescriptorBufferInfoBuilder::new()
                            .range(65536)
                            .buffer(glyphs.handle)
                            .offset(0)]),
                    vk::WriteDescriptorSetBuilder::new()
                        .dst_binding(1)
                        .dst_set(dset)
                        .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
                        .buffer_info(&[vk::DescriptorBufferInfoBuilder::new()
                            .range(32768)
                            .buffer(transforms.handle)
                            .offset(0)]),
                ],
                &[],
            );
        }

        buffers[i] = PerFrameBuffers { glyphs, transforms };
    }

    let mut index_buffer = vk.allocator.allocate_buffer(
        &vk.device,
        &BufferAllocation {
            size: 4096 * 6 * 2, // max glyphs*indices per glyph*sizeof(u16)
            usage: UsageFlags::FAST_DEVICE_ACCESS,
            vk_usage: vk::BufferUsageFlags::INDEX_BUFFER | vk::BufferUsageFlags::TRANSFER_DST,
        },
    )?;

    let mut ibo_contents: Vec<u16> = Vec::with_capacity(4096 * 6);
    // 0,1,2  1,2,3  4,5,6  5,6,7 ...
    for i in 0..4096 * 6 as u16 {
        let value = if i % 6 > 2 { i - 2 } else { i };
        ibo_contents.push(value - i / 6 * 2);
    }
    let ibo_content_bytes: &[u8] = bytemuck::cast_slice(&ibo_contents);
    vk.uploader
        .upload_bytes_to_buffer(&vk.device, ibo_content_bytes, &mut index_buffer, 0)?;

    Ok(TextRenderer {
        rendering: RenderResources {
            index_buffer,
            buffers,
        },
        text_buffer,
        transform_buffer,

        scissors: Vec::with_capacity(32),
        current_scissor_area: vk::Rect2D {
            offset: vk::Offset2D { x: 0, y: 0 },
            extent: vk.swapchain.surface.extent,
        },
        current_scissor_start: 1,

        viewport_size: vk.swapchain.surface.extent,
        proj_view,

        glyphs,
    })
}

/*use freetype as ft;

fn gen_files() -> Result<Box<[GlyphData; 256]>> {
    let bytes = lz4::block::decompress(assets::fonts::GRAND9K, None)?;

    let lib = ft::Library::init()?;
    let face = lib.new_memory_face(bytes, 0)?;

    println!(
        "Height: {}, num glyphs: {}, num faces: {}, scalable: {}",
        face.height(),
        face.num_glyphs(),
        face.num_faces(),
        face.is_scalable()
    );
    println!("Style flags: {:?}", face.style_flags());
    println!("Global height: {}", face.ascender() - face.descender());
    println!("Metrics: {:?}", face);
    println!("Units per EM: {}", face.em_size());

    face.set_pixel_sizes(8, 8).unwrap();

    println!(
        "Height: {}, num glyphs: {}, num faces: {}, scalable: {}, has_fixed_sizes: {}, metrics; {:?}",
        face.height(),
        face.num_glyphs(),
        face.num_faces(),
        face.is_scalable(),
        face.has_fixed_sizes(),
        face.size_metrics()
    );
    println!("Style flags: {:?}", face.style_flags());
    println!("Global height: {}", face.ascender() - face.descender());
    println!("Kerning: {}", face.has_kerning());

    // SIGH. TODO find a way to actually get 275 characters without needing to guess what's needed
    const CHARS: &'static str = "\
    abcdefghijklmnopqrstuvwxyzåäö\
    ABCDEFGHIJKLMNOPQRSTUVWXYZÅÄÖ\
    1234567890\
    $€£+*-/÷=%\"'#@&_(),.;:?!\\|{}<>[]§`^~ \
    ";

    let mut data = vec![0u8; (8 * 8) * (16 * 16) as usize];
    let mut lut = Box::new([GlyphData::default(); 256]);

    let mut count = 0;
    let mut max_width = 0;
    let mut max_height = 0;
    let mut glyph_x = 0;
    let mut glyph_y = 0;

    let mut min_y_offset = 0;
    let mut max_y_offset = 0;

    let mut used: HashSet<usize> = HashSet::<usize>::new();

    for c in CHARS.chars() {
        let gindex = face.get_char_index(c as usize);
        if gindex == 0 {
            continue;
        }
        print!("Loading '{}'! ", c);

        face.load_glyph(gindex, ft::face::LoadFlag::empty())?;
        let glyph = face.glyph();

        glyph.render_glyph(ft::RenderMode::Normal)?;
        let bm = glyph.bitmap();
        let mut buffer_idx = 0usize;
        let pitch = bm.pitch() as usize;
        let mut base_idx = (glyph_x * 8) + (glyph_y * 8) * (8 * 16 * 2); // glyph width * glyph grid width = canvas width
        for _ in 0..bm.rows() {
            data[base_idx..(base_idx + pitch)]
                .copy_from_slice(&bm.buffer()[buffer_idx..(buffer_idx + pitch)]);
            buffer_idx += pitch;
            base_idx += 16 * 8;
        }
        glyph_x += 1;
        if glyph_x == 16 {
            glyph_x = 0;
            glyph_y += 1;
        }

        max_width = max_width.max(bm.width());
        max_height = max_height.max(bm.rows());

        println!("{}x{}", bm.width(), bm.rows());

        if glyph.bitmap_left() != 0 {
            println!("ZING ZING ZING");
        }

        let metrics = glyph.metrics();

        let yoff = ((-metrics.height + metrics.horiBearingY) / 64) as i8; // as f32 / (64.0 * 8.0);
        min_y_offset = i8::min(min_y_offset, yoff);
        max_y_offset = i8::min(max_y_offset, yoff);
        let width = i32::max(bm.width(), (metrics.horiAdvance / 64) as i32);
        //println!("Width: {}, height: {}, YOff: {}", width, bm.rows(), yoff);
        println!(
            "advance: {}, yoff: {yoff}",
            metrics.horiAdvance as f64 / 64.0
        );
        lut[c as usize & 255] = GlyphData {
            char: c as u32,
            base_and_dims: (((2 + yoff) as u16) << 7) | ((width as u16) << 4) | (bm.rows() as u16),
            layer: count,
            advance: (metrics.horiAdvance / 64) as u8, //) as f32 / 8.0,
        };
        if !used.insert((c as usize) & 0xFF) {
            panic!("DUPLICATE MAPPING: {}", c as usize);
        }

        count += 1;
    }

    println!(
        "Found {}/{} glyphs. Max width: {}, height: {}",
        count,
        CHARS.len(),
        max_width,
        max_height
    );
    println!("Min/max y offsets: {}, {}", min_y_offset, max_y_offset);

    let mut encoder = png::Encoder::new(File::create("font_array.png").unwrap(), 16 * 8, 16 * 8);
    encoder.set_color(png::ColorType::Grayscale);
    encoder.set_depth(png::BitDepth::Eight);

    let mut output_file = File::create("font_atlas.dat").unwrap();
    output_file.write_all(&data).unwrap();

    let glyph_bytes: &[u8] = bytemuck::cast_slice(&lut[..]);
    let mut output_file = File::create("glyph_info.dat").unwrap();
    output_file.write_all(&glyph_bytes).unwrap();

    let mut writer = encoder.write_header().unwrap();
    writer.write_image_data(&data).unwrap();

    Ok(lut)
}*/
