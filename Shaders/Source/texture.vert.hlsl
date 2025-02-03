struct SpriteData {
    float3 Position;
    float Rotation;
    float2 Scale;
    float2 Padding;
    float TexU, TexV, TexW, TexH;
    float4 Color;
};

struct Output {
    float2 Texcoord : TEXCOORD0;
    float4 Color    : TEXCOORD1;
    float4 Position : SV_Position;
};

StructuredBuffer<SpriteData> DataBuffer : register(t0, space0);

cbuffer UniformBlock : register(b0, space1) {
    float4x4 ViewProjectionMatrix : packoffset(c0);
};

// Triangle indices for a quad (six vertices)
static const uint triangleIndices[6] = { 0, 1, 2, 3, 2, 1 };

Output main(uint id : SV_VertexID) {
    // Determine sprite and vertex (for that sprite)
    uint spriteIndex = id / 6;
    uint triIndex    = id % 6;
    uint vert        = triangleIndices[triIndex];

    SpriteData sprite = DataBuffer[spriteIndex];

    // Compute local quad coordinates from bit values
    float2 localPos = float2((vert & 1), (vert >> 1));  

    // Inline texcoord computation (quad defined over [0, 1] x [0, 1])
    float2 texcoord = float2(
        sprite.TexU + sprite.TexW * localPos.x,
        sprite.TexV + sprite.TexH * localPos.y
    );

    // Scale quad coordinates
    localPos *= sprite.Scale;

    // Compute rotation - same for all vertices of the sprite
    float s = sin(sprite.Rotation);
    float c = cos(sprite.Rotation);
    localPos = float2(localPos.x * c - localPos.y * s,
                       localPos.x * s + localPos.y * c);

    // Final world position (preserving depth)
    float3 worldPos = float3(sprite.Position.xy + localPos, sprite.Position.z);

    Output output;
    output.Position = mul(ViewProjectionMatrix, float4(worldPos, 1.0));
    output.Texcoord = texcoord;
    output.Color    = sprite.Color;
    return output;
}
