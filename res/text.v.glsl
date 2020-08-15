#version 330 core

in vec2 gridCoords;         // Gird Coordinate
in vec4 uvAttr;             // Physical pixel Coordinate
in float baseline;          // Physical pixel Coordinate

out vec2 TexCoords;         // Texture Coordinate

uniform vec2 cellSize;      // NDC Coordinate
uniform vec2 windowSize;    // Physical pixel Coordinate
uniform int drawFlag;

void main() {
    // Converts the grid coordinates to the position of the cell-top-left in the NDC coordinates.
    // Step 1, reverse y-axis direction:    * vec2(1, -1)
    // Step 2, scaling to NDC Coordinate:   * cellSize
    // Step 3, move to the top-left:        + vec(-1, 1)
    vec2 cellPosition = gridCoords * vec2(1, -1) * cellSize + vec2(-1, 1);

    // Compute vertex position of four corners, clockwise, from the top-right.
    vec2 position;
    position.x = (gl_VertexID == 0 || gl_VertexID == 1) ? 1.0 : 0.0;
    position.y = (gl_VertexID == 0 || gl_VertexID == 3) ? 0.0 : -1.0;

    // Get glyph position information
    vec2 uvSize = uvAttr.xy / windowSize.xy;
    vec2 uvOffset = uvAttr.zw / windowSize.xy - vec2(0, cellSize.y);

    // Vertex real position, NDC Coordinate
    vec2 vertexPosition;
    if (drawFlag == 0 || drawFlag == 1) {   // draw texture(0) or bounding box(1)
        vertexPosition = cellPosition + uvOffset + position * uvSize;
    } else if (drawFlag == 2) {             // draw cell(2)
        vertexPosition = cellPosition + position * cellSize;
    } else {                                // draw baseline(3)
        vertexPosition = cellPosition + position * cellSize * vec2(1, 0) + vec2(0, baseline/windowSize.y);
    }

    gl_Position = vec4(vertexPosition, 0.0, 1.0);

    // Compute texture corner position
    TexCoords = position * vec2(1.0, -1.0);
}
