#version 450
#extension GL_ARB_separate_shader_objects : enable

layout (location = 0) in vec3 aPosition;
layout (location = 1) in uvec3 aColor;

layout(location = 0) out vec3 fragColor;

layout (push_constant) uniform constants {
    mat4 projection;
} pushConstants;

void main() {
    gl_Position = pushConstants.projection * vec4(aPosition, 1.0);
    fragColor = vec3(aColor);
}
