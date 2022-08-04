#version 460
#extension GL_ARB_separate_shader_objects : enable

layout(location = 0) in uint xy;
layout(location = 1) in uint color_or_uv;

layout(location = 0) out vec4 color;

layout (push_constant) uniform constants {
    vec2 scale;
} pushConstants;

void main() {
    gl_Position = vec4(vec2(-1.0) + pushConstants.scale * vec2(xy & 0xFFFF, (xy >> 16) & 0xFFFF), 0, 1);
    color = unpackUnorm4x8(color_or_uv), vec4(2.2);
}

