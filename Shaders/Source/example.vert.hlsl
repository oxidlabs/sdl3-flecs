struct Input {
    float3 Position : SDL_GPUPOSITION;
};

struct Output {
    float4 Position : SV_Position;
};

Output main(Input input) {
    Output output;
    output.Position = float4(input.Position, 1.0f);
    return output;
}