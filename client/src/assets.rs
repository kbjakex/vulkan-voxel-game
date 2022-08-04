
// The file where all include_bytes!() invocations shall lie
// because the path for include_bytes! is relative.

macro_rules! include_asset {
    ($path:expr) => {
        include_bytes!(concat!("../assets/", $path))
    };
}

macro_rules! include_shader {
    ($shader_name:literal) => {{
        #[cfg(not(debug_assertions))] { include_asset!(concat!("shaders/bin/", $shader_name, ".spv")) }
        #[cfg(    debug_assertions )] { include_asset!(concat!("shaders/bin/debug_", $shader_name, ".spv")) }
    }};
}

pub mod terrain_pipeline {
    pub const TERRAIN_SHADER_VERT: &[u8] = include_shader!("triangle.vert");
    pub const TERRAIN_SHADER_FRAG: &[u8] = include_shader!("triangle.frag");
}

pub mod text {
    pub const TEXT_SHADER_VERT: &[u8] = include_shader!("text.vert");
    pub const TEXT_SHADER_FRAG: &[u8] = include_shader!("text.frag");
    pub const TEXTURE_ATLAS : &[u8] = include_asset!("fonts/font_atlas.bin");
    pub const GLYPH_INFO : &[u8] = include_asset!("fonts/glyph_info.bin");
}

pub mod postprocess_pipelines {
    pub const FULLSCREEN_SHADER_VERT: &[u8] = include_shader!("fullscreen.vert");
    /* pub const SKY_SHADER_FRAG: &[u8] = include_shader!("sky.frag"); */
    pub const LUMA_SHADER_FRAG: &[u8] = include_shader!("luminance.frag");
    pub const FXAA_SHADER_FRAG: &[u8] = include_shader!("fxaa.frag");
}

pub mod ui_pipeline {
    pub const IMMEDIATE_MODE_SHADER_VERT: &[u8] = include_shader!("immediate.vert");
    pub const IMMEDIATE_MODE_SHADER_FRAG: &[u8] = include_shader!("immediate.frag");
}

pub mod textures {
    // Lz4-HC compressed
    pub const TEXTURES: &[u8] = include_asset!("textures/packed.bin");
}


/* pub mod fonts {
    pub const TINYUNICODE: &[u8] = include_asset!("fonts/TinyUnicode.bin");
    pub const GRAND9K: &[u8] = include_asset!("fonts/grand9k.bin");
} */
