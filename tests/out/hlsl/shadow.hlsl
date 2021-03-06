static const float3 c_ambient = float3(0.05, 0.05, 0.05);

static const uint c_max_lights = 10;

struct Globals {
    uint4 num_lights;
};

struct Light {
    float4x4 proj;
    float4 pos;
    float4 color;
};

struct Lights {
    Light data[1];
};

cbuffer u_globals : register(b0) { Globals u_globals; }
Lights s_lights : register(t1);
Texture2DArray t_shadow : register(t2);
SamplerComparisonState sampler_shadow : register(s3);

struct FragmentInput_fs_main {
    float3 raw_normal1 : LOC0;
    float4 position1 : LOC1;
};

float fetch_shadow(uint light_id, float4 homogeneous_coords)
{
    if ((homogeneous_coords.w <= 0.0)) {
        return 1.0;
    }
    float2 flip_correction = float2(0.5, -0.5);
    float2 light_local = ((mul(homogeneous_coords.xy, flip_correction) / float2(homogeneous_coords.w.xx)) + float2(0.5, 0.5));
    float _expr26 = t_shadow.SampleCmpLevelZero(sampler_shadow, float3(light_local, int(light_id)), (homogeneous_coords.z / homogeneous_coords.w));
    return _expr26;
}

float4 fs_main(FragmentInput_fs_main fragmentinput_fs_main) : SV_Target0
{
    float3 color = float3(0.05, 0.05, 0.05);
    uint i = 0u;

    float3 normal = normalize(fragmentinput_fs_main.raw_normal1);
    while(true) {
        uint _expr12 = i;
        uint4 _expr14 = u_globals.num_lights;
        if ((_expr12 >= min(_expr14.x, c_max_lights))) {
            break;
        }
        uint _expr19 = i;
        Light light = s_lights.data[_expr19];
        uint _expr22 = i;
        const float _e25 = fetch_shadow(_expr22, mul(light.proj, fragmentinput_fs_main.position1));
        float3 light_dir = normalize((light.pos.xyz - fragmentinput_fs_main.position1.xyz));
        float diffuse = max(0.0, dot(normal, light_dir));
        float3 _expr34 = color;
        color = (_expr34 + mul(mul(_e25, diffuse), light.color.xyz));
        uint _expr40 = i;
        i = (_expr40 + 1u);
    }
    float3 _expr43 = color;
    return float4(_expr43, 1.0);
}
