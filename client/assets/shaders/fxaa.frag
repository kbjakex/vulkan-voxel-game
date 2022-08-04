#version 450
#extension GL_ARB_separate_shader_objects : enable

layout(location = 0) in vec2 uv;

layout(location = 0) out vec4 outColor;

layout(set = 1, binding = 0) uniform sampler2D texColor;
layout(set = 1, binding = 1) uniform sampler2D texLuma;
layout(set = 1, binding = 2) uniform UBO {
	vec2 texelSize;
} ubo;

float getLuminance(vec2 offset) {
    vec2 tc = uv + offset * ubo.texelSize;
    return texture(texLuma, tc).r;
}

const int extraEdgeSteps = 3;
const float edgeStepSizes[3] = { 1.5, 2.0, 2.0 };
const float lastEdgeStepGuess = 8.0;

void main() {
	/* outColor = texture(texColor, uv);
	return; */

    float lum = texture(texLuma, uv).x;

    const float subpixelBlending = 0.80;

    float m = getLuminance(vec2(0.0));
	float n = getLuminance(vec2(0.0, 1.0));
	float e = getLuminance(vec2(1.0, 0.0));
	float s = getLuminance(vec2(0.0, -1.0));
	float w = getLuminance(vec2(-1.0, 0.0));
    float ne = getLuminance(vec2(1.0, 1.0));
	float se = getLuminance(vec2(1.0, -1.0));
	float sw = getLuminance(vec2(-1.0, -1.0));
	float nw = getLuminance(vec2(-1.0, 1.0));

	float highest = max(max(max(max(m, n), e), s), w);
	float lowest = min(min(min(min(m, n), e), s), w);
    float range = highest - lowest;

    if (range < max(0.0625, 0.166 * highest)) { outColor = texture(texColor, uv); return; }

    float factor = abs(m -  0.0833333333 * (2.0 * (n + e + s + w) + ne + se + sw + nw)) / range;
    factor = smoothstep(0.0, 1.0, clamp(factor, 0.0, 1.0));
    factor = factor * factor * subpixelBlending;

    float horizontal =
		2.0 * abs(n + s - 2.0 * m) +
		abs(ne + se - 2.0 * e) +
		abs(nw + sw - 2.0 * w);
	float vertical =
		2.0 * abs(e + w - 2.0 * m) +
		abs(ne + nw - 2.0 * n) +
		abs(se + sw - 2.0 * s);

    bool isHor = horizontal >= vertical;

	float lumaP, lumaN;
    float pixelStep;
	if (isHor) {
		pixelStep = ubo.texelSize.y;
		lumaP = n;
		lumaN = s;
	}
	else {
		pixelStep = ubo.texelSize.x;
		lumaP = e;
		lumaN = w;
	}
	float gradientP = abs(lumaP - m);
	float gradientN = abs(lumaN - m);

    float lumaGradient, otherLuma;
	if (gradientP < gradientN) {
		pixelStep = -pixelStep;
        lumaGradient = gradientN;
        otherLuma = lumaN;
	} else {
        lumaGradient = gradientP;
        otherLuma = lumaP;
    }

    vec2 edgeUV = uv;
    vec2 uvStep = vec2(0.0);
    if (isHor) {
        edgeUV.y += 0.5 * pixelStep;
        uvStep.x = ubo.texelSize.x;
    } else {
        edgeUV.x += 0.5 * pixelStep;
        uvStep.y = ubo.texelSize.y;
    }
    float edgeLuma = 0.5 * (m + otherLuma);
	float gradientThreshold = 0.25 * lumaGradient;
	vec2 uvP = edgeUV + uvStep;
	float lumaDeltaP = texture(texLuma, uvP).r - edgeLuma;
	bool atEndP = abs(lumaDeltaP) >= gradientThreshold;

	for (int i = 0; i < extraEdgeSteps && !atEndP; i++) {
		uvP += uvStep * edgeStepSizes[i];
		lumaDeltaP = texture(texLuma, uvP).r - edgeLuma;
		atEndP = abs(lumaDeltaP) >= gradientThreshold;
	}
	if (!atEndP) {
		uvP += uvStep * lastEdgeStepGuess;
	}

	vec2 uvN = edgeUV - uvStep;
	float lumaDeltaN = texture(texLuma, uvN).r - edgeLuma;
	bool atEndN = abs(lumaDeltaN) >= gradientThreshold;

	for (int i = 0; i < extraEdgeSteps && !atEndN; i++) {
		uvN -= uvStep * edgeStepSizes[i];
		lumaDeltaN = texture(texLuma, uvN).r - edgeLuma;
		atEndN = abs(lumaDeltaN) >= gradientThreshold;
	}
	if (!atEndN) {
		uvN -= uvStep * lastEdgeStepGuess;
	}

    float distanceToEndP, distanceToEndN;
	if (isHor) {
		distanceToEndP = uvP.x - uv.x;
		distanceToEndN = uv.x - uvN.x;
	}
	else {
		distanceToEndP = uvP.y - uv.y;
		distanceToEndN = uv.y - uvN.y;
	}
	float distanceToNearestEnd;
	bool deltaSign;
	if (distanceToEndP <= distanceToEndN) {
		distanceToNearestEnd = distanceToEndP;
		deltaSign = lumaDeltaP >= 0;
	}
	else {
		distanceToNearestEnd = distanceToEndN;
		deltaSign = lumaDeltaN >= 0;
	}
    float edgeBlendFactor;
    if (deltaSign == (m - edgeLuma >= 0)) {
        edgeBlendFactor = 0.0;
    } else {
        edgeBlendFactor = 0.5 - distanceToNearestEnd / (distanceToEndP + distanceToEndN);
    }
    
    float blendFactor = max(factor, edgeBlendFactor);
	vec2 blendUV = uv;
	if (isHor) {
		blendUV.y += blendFactor * pixelStep;
	}
	else {
		blendUV.x += blendFactor * pixelStep;
	}
	outColor = texture(texColor, blendUV);
}
