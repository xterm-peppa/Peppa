#version 330 core

in vec2 TexCoords;

out vec4 color;
out vec4 alphaMask;

uniform sampler2D ourTexture;

uniform int drawFlag;

void main() {
    if (drawFlag == 0) {
        color = texture(ourTexture, TexCoords) * vec4(1.0f, 1.0f, 1.0f, 1.0f);
    } else if (drawFlag == 1) {
        color = vec4(0.0f, 1.0f, 1.0f, 1.0f);
    } else if (drawFlag == 2) {
        color = vec4(0.5f, 0.5f, 0.0f, 1.0f);
    } else {
        color = vec4(1.0f, 0.0f, 1.0f, 1.0f);
    }
}
