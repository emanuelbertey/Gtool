#[compute]
#version 450

struct u64x {
	uint low;
	uint high;
};

layout(set = 0, binding = 0, std430) restrict readonly buffer TargetValues {
	uint target_values[];
};

layout(set = 0, binding = 1, std430) restrict buffer TargetHashes {
	uint target_hash16_values[];
};

layout(push_constant, std430) uniform Params {
	uint seed0_low;
	uint seed0_high;
	uint seed1_low;
	uint seed1_high;
	uint target_count;
	uint _padding0;
	uint _padding1;
	uint _padding2;
} params;

layout(local_size_x = 256, local_size_y = 1, local_size_z = 1) in;

u64x add64(u64x a, u64x b) {
	u64x res;
	uint low_sum = a.low + b.low;
	uint carry = low_sum < a.low ? 1u : 0u;
	res.low = low_sum;
	res.high = a.high + b.high + carry;
	return res;
}

u64x xor64(u64x a, u64x b) {
	return u64x(a.low ^ b.low, a.high ^ b.high);
}

u64x rotl64(u64x v, uint shift) {
	shift &= 63u;
	if (shift == 0u) {
		return v;
	}
	if (shift == 32u) {
		return u64x(v.high, v.low);
	}
	if (shift < 32u) {
		return u64x(
			(v.low << shift) | (v.high >> (32u - shift)),
			(v.high << shift) | (v.low >> (32u - shift))
		);
	}
	shift -= 32u;
	return u64x(
		(v.high << shift) | (v.low >> (32u - shift)),
		(v.low << shift) | (v.high >> (32u - shift))
	);
}

void sip_round(inout u64x v0, inout u64x v1, inout u64x v2, inout u64x v3) {
	v0 = add64(v0, v1);
	v2 = add64(v2, v3);
	v1 = rotl64(v1, 13u);
	v3 = rotl64(v3, 16u);
	v1 = xor64(v1, v0);
	v3 = xor64(v3, v2);
	v0 = rotl64(v0, 32u);

	v2 = add64(v2, v1);
	v0 = add64(v0, v3);
	v1 = rotl64(v1, 17u);
	v3 = rotl64(v3, 21u);
	v1 = xor64(v1, v2);
	v3 = xor64(v3, v0);
	v2 = rotl64(v2, 32u);
}

u64x siphash13_one_u32(uint value) {
	u64x k0 = u64x(params.seed0_low, params.seed0_high);
	u64x k1 = u64x(params.seed1_low, params.seed1_high);

	u64x v0 = xor64(k0, u64x(0x70736575u, 0x736f6d65u));
	u64x v1 = xor64(k1, u64x(0x6e646f6du, 0x646f7261u));
	u64x v2 = xor64(k0, u64x(0x6e657261u, 0x6c796765u));
	u64x v3 = xor64(k1, u64x(0x79746573u, 0x74656462u));

	u64x m = u64x(value, 0x04000000u);
	v3 = xor64(v3, m);
	sip_round(v0, v1, v2, v3);
	v0 = xor64(v0, m);

	v2 = xor64(v2, u64x(0x000000ffu, 0u));
	sip_round(v0, v1, v2, v3);
	sip_round(v0, v1, v2, v3);
	sip_round(v0, v1, v2, v3);

	return xor64(xor64(v0, v1), xor64(v2, v3));
}

void main() {
	uint id = gl_GlobalInvocationID.x;
	if (id >= params.target_count) {
		return;
	}

	u64x hash64 = siphash13_one_u32(target_values[id]);
	target_hash16_values[id] = hash64.low & 0xffffu;
}
