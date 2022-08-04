#version 460
#extension GL_ARB_separate_shader_objects : enable

layout(location = 0) out vec4 finalColor;

layout(location = 0) in vec4 color;

void main() {
    finalColor = color;
}

