#version 450
#extension GL_ARB_separate_shader_objects : enable

layout(location = 0) in vec3 color;
layout(location = 1) in vec3 pos;

layout(location = 0) out vec4 outColor;

layout(set = 0, binding = 0) uniform sampler2DArray tex1;

float rand(vec2 co){
    return fract(sin(dot(co, vec2(12.9898, 78.233))) * 43758.5453);
}

float rand3(vec3 co) {
    return rand(vec2(rand(co.xy), rand(co.yz)));
}

void main() {
    vec2 pos = floor(pos.xz);
    float col = mod(pos.x + pos.y, 2.0) + 6.0;
    outColor = vec4(texture(tex1, vec3(color.xy, col)).rgb, 1.0);
    //outColor = texture(tex1, vec3(color.xy, rand3(floor(pos* 0.9999)) * 16.0));
}
