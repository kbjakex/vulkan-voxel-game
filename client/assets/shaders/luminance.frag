#version 450
#extension GL_ARB_separate_shader_objects : enable

layout(location = 0) in vec2 uv;

layout(location = 0) out float outColor;

layout(set = 1, binding = 0) uniform sampler2D texColor;

float linearRgbToLuminance(vec3 linearRgb) {
	return dot(linearRgb, vec3(0.2126729,  0.7151522, 0.0721750));
}

void main() {
    vec3 s = texture(texColor, uv).xyz;
    float lum = linearRgbToLuminance(s);
    outColor = lum;
}
