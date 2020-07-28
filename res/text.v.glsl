#version 330 core

in vec2 gridCoords;
in vec4 uvAttr;
in float baseline;

out vec2 TexCoords;

uniform vec2 cellSize;
uniform int drawFlag;

void main() {
    // Position of cell from top-left
    vec2 cellPosition = gridCoords * vec2(1, -1) * cellSize + vec2(-1, 1);

    // Compute vertex corner position
    vec2 position;
    position.x = (gl_VertexID == 0 || gl_VertexID == 1) ? 1.0 : 0.0;
    position.y = (gl_VertexID == 0 || gl_VertexID == 3) ? 0.0 : -1.0;

    vec2 uvSize = uvAttr.xy;
    vec2 uvOffset = uvAttr.zw;

    // Vertex real position
    vec2 vertexPosition;
    if (drawFlag == 0 || drawFlag == 1) {
        vertexPosition = cellPosition + uvOffset + position * uvSize;
    } else if (drawFlag == 2) {
        vertexPosition = cellPosition + position * cellSize;
    } else if (drawFlag == 3) {
        vertexPosition = cellPosition + vec2(position.x, uvOffset.y - uvSize.y + baseline);
    }

    gl_Position = vec4(vertexPosition, 0.0, 1.0);

    // Compute texture corner position
    TexCoords = position * vec2(1.0, -1.0);
}
