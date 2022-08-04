#version 450
#extension GL_ARB_separate_shader_objects : enable

layout(location = 0) in vec3 aPos;
layout(location = 1) in vec3 aCol;
layout(location = 2) in vec2 aUV;

layout(location = 0) out vec3 color;
layout(location = 1) out vec3 pos;

layout (push_constant) uniform constants {
    mat4 projection;
} pushConstants;

//layout(set = 1, binding = 0) uniform  CameraBuffer{
//	mat4 pv;
//} cameraData;

void main() {
    gl_Position = pushConstants.projection * vec4(aPos, 1.0);
    color = vec3(aUV, 0.0);
    pos = aPos;
}

