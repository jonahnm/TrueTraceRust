#include "../../GlobalDefines.cginc"
#ifndef DX11
    #pragma use_dxc
    // #pragma enable_d3d11_debug_symbols
#endif
#include "../CommonData.cginc"
#ifdef HardwareRT
   // #include "UnityRayQuery.cginc"
   // #pragma require inlineraytracing
    RaytracingAccelerationStructure myAccelerationStructure;
#endif
static float g = sin(atan(1.0f / 2.0f));
RWTexture2D<uint4> _PrimaryTriangleInfo;
bool GetDist(float3 CurrentPos, out float2 uv, out float dist, const TerrainData Terrain) {
    float3 b = float3(Terrain.TerrainDim.x, 0.01f, Terrain.TerrainDim.y);
    float3 q = (abs(CurrentPos) - b);
    q.x /= Terrain.TerrainDim.x;
    q.z /= Terrain.TerrainDim.y;
    uv = float2(min(CurrentPos.x / Terrain.TerrainDim.x, 1), min(CurrentPos.z / Terrain.TerrainDim.y, 1));
    float h = Heightmap.SampleLevel(sampler_trilinear_clamp, uv * (Terrain.HeightMap.xy - Terrain.HeightMap.zw) + Terrain.HeightMap.zw, 0).x;
    h *= Terrain.HeightScale * 2.0f;
    q.y -= h;
    q.y *= g;
    float b2 = q.y;
    q = max(0, q);
    dist = length(q);
    return b2 != abs(b2);
}

inline bool rayBoxIntersection(const float3 ray_orig, const float3 ray_dir, const float3 Min, const float3 Max, float tMax, inout float t0) {
    const float3 tmp_min = (Min - ray_orig) / ray_dir;
    const float3 tmp_max = (Max - ray_orig) / ray_dir;
    const float3 tmin = min(tmp_min, tmp_max);
    const float3 tmax = max(tmp_min, tmp_max);
    t0 = max(tmin.x, max(tmin.y, max(tmin.z, 0.025f))); // Usually ray_tmin = 0
    float t1 = min(tmax.x, min(tmax.y, min(tmax.z, tMax)));
    return (t0 <= t1);
}