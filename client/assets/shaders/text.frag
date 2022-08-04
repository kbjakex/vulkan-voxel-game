#version 460
#extension GL_ARB_separate_shader_objects : enable

layout(location = 0) in vec2 uv;
layout(location = 1) in vec4 color;

layout(location = 0) out vec4 outColor;

layout(set = 0, binding = 1) uniform sampler2D texGlyphs;

void main() {
    vec4 col = texture(texGlyphs, uv).r * color;
    if (col.w == 0.0) {
        discard;
    }

    outColor = col;
}
