#version 460
#extension GL_ARB_separate_shader_objects : enable

layout(location = 0) out vec2 uv;
layout(location = 1) out vec4 color;

struct Glyph { 
    // 1b is_3d, 3b 0..7 layer_y, 4b 0..15 layer_x
    // if is_3d: 9b 0..512 transform_idx, 7b 0..128 x
    // else:     12b 0..4096 y, 12b 0..4096 x
    uint d1;
    // (22b R6G6B6A3 color << 11) | (1b italic << 10) | (3b base y << 7) | (3b 0..7 dimX << 4) | 4b 0..15 dimY
    uint d2;
};

layout(set = 1, binding = 0) readonly buffer GlyphData {
    vec2 scale;
	Glyph arr[8191]; // up to 8191 total glyphs on screen at once
} glyphs;

layout(set = 1, binding = 1) uniform PosData {
	mat4 arr[2048]; // up to 2048 transforms at once
} positions;

void main() {
    vec2 offset = vec2(gl_VertexIndex & 1, (gl_VertexIndex >> 1) & 1);
    
    Glyph glyph = glyphs.arr[gl_VertexIndex >> 2];
    uint d1 = glyph.d1, d2 = glyph.d2;

    // base: distance (in pixels) from glyph bottom to baseline. 0 for most glyphs.
    // Negative for glyphs that reach below the baseline (g, j, y, q, p, ...) and positive
    // for glyphs above, e.g. ^ ' ~ -
    float base = float((d2 >> 7) & 7) * 3.0 - 2.0*3.0; // shift back from 0..=7 to -2..=5
    vec2 dim = vec2((d2 >> 4) & 7, d2 & 15); // in pixels
    vec2 vertex_pos = offset * dim * 3.0; // in pixels. *3 is the currently hardcoded scale.

    if ((d1 >> 31) == 0) { // if !is_3d
        vertex_pos += vec2(float(d1 & 0xFFF), float((d1 >> 12) & 0xFFF) + base);
    } else { // if is_3d
        vertex_pos.x += float(d1 & 0x7F);
    }

    // Apply italic if bit 7 (0-indexed) is set
    if ((d2 & 0x400) != 0) {
        // 10 degree slanting: tan(10 deg) = 0.17632698
        // offset.y == 1.0 if top vertex, 0.0 if bottom vertex
        vertex_pos.x += (3.0 * 0.17632698) * mix(base, dim.y + base, offset.y);
    }

    offset.y = 1.0 - offset.y;
    
    // Each glyph in atlas is reserved a 8x16 space, so
    // these units are in "glyph coordinate space", as in,
    // 8 pixels per x unit and 16 pixels per y unit.
    // There are 16 glyphs per row and 8 per column,
    // so dividing by (16, 8) is enough to produce UVs.
    // `offset * dim` otoh is in pixels, and there are
    // 16*8 x 8*16 <=> 128x128 pixels in the atlas.
    //        vvvvvvvvvvvvvvv  vvvvvvvvvvvvvv
    uv = vec2((d1 >> 24) & 15, (d1 >> 28) & 7) * vec2(1.0 / 16.0, 1.0 / 8.0) + offset * dim * (1.0 / 128.0);
    gl_Position = vec4(vertex_pos, 0.0, 1.0);

    if ((d1 >> 31) == 0) { // if is_3d
        gl_Position = vec4(glyphs.scale, 1.0, 1.0) * gl_Position - vec4(1.0, 1.0, 0.0, 0.0);
    } else {
        gl_Position = positions.arr[(d1 >> 7) & 0x1FF] * gl_Position;
    }

    //d2 = 0xFFFFFFFF;

    // RGB are 6-bit aka 0..63. Alpha is 4-bit aka 0..15.
    color = vec4(
        (d2 >> 26) & 0x3F, 
        (d2 >> 20) & 0x3F, 
        (d2 >> 14) & 0x3F, 
        (d2 >> 11) & 0x07) * vec4(vec3(1.0 / 63.0), 1.0 / 7.0);
}

