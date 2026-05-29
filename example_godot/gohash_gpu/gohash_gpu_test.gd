extends Node

const TOTAL_VALUES := 16_777_216
const WORKGROUP_SIZE := 256
const START_VALUE := 0
const TARGET_VALUE := 1_234_567
const SEED0_LOW := 0
const SEED0_HIGH := 0
const SEED1_LOW := 0
const SEED1_HIGH := 0
const NOT_MATCH := 0xffffffff
const SHADER_PATH := "res://example_godot/gohash_gpu/siphash16_bruteforce.glsl"
const PREPARE_SHADER_PATH := "res://example_godot/gohash_gpu/siphash16_prepare_targets.glsl"
const MULTI_SHADER_PATH := "res://example_godot/gohash_gpu/siphash16_multi_targets.glsl"
const MULTI_TARGET_COUNT := 1024
const MULTI_TARGET_BATCH_SIZE := 16

func _ready() -> void:
	run_gpu_gohash_test()
	run_gpu_cpu_hash_match_test()
	run_gpu_multi_target_test()

func run_gpu_gohash_test() -> void:
	var shader_file := load(SHADER_PATH)
	if shader_file == null:
		push_error("No se pudo cargar el shader: %s" % SHADER_PATH)
		return

	var spirv: RDShaderSPIRV = shader_file.get_spirv()
	if spirv == null:
		push_error("El shader no devolvio SPIR-V. Revisa que sea #[compute] #version 450.")
		return

	var rd := RenderingServer.create_local_rendering_device()
	if rd == null:
		push_error("RenderingDevice local no disponible. Compute shader requiere backend Vulkan; el proyecto esta en GL Compatibility.")
		return

	var shader := rd.shader_create_from_spirv(spirv)
	if not shader.is_valid():
		push_error("Shader compute invalido o no compilado.")
		return

	var target_hash16 := _cpu_siphash13_u32_hash16(TARGET_VALUE, SEED0_LOW, SEED0_HIGH, SEED1_LOW, SEED1_HIGH)
	var output_size := TOTAL_VALUES * 4
	var output_bytes := PackedByteArray()
	output_bytes.resize(output_size)
	output_bytes.fill(0xff)
	var output_buffer := rd.storage_buffer_create(output_size, output_bytes)

	var uniform := RDUniform.new()
	uniform.uniform_type = RenderingDevice.UNIFORM_TYPE_STORAGE_BUFFER
	uniform.binding = 0
	uniform.add_id(output_buffer)

	var uniform_set := rd.uniform_set_create([uniform], shader, 0)
	var pipeline := rd.compute_pipeline_create(shader)
	var push_constants := _make_push_constants(target_hash16)
	var groups_x := int(ceil(float(TOTAL_VALUES) / float(WORKGROUP_SIZE)))

	var start_usec := Time.get_ticks_usec()
	var compute_list := rd.compute_list_begin()
	rd.compute_list_bind_compute_pipeline(compute_list, pipeline)
	rd.compute_list_bind_uniform_set(compute_list, uniform_set, 0)
	rd.compute_list_set_push_constant(compute_list, push_constants, push_constants.size())
	rd.compute_list_dispatch(compute_list, groups_x, 1, 1)
	rd.compute_list_end()
	rd.submit()
	rd.sync()
	var elapsed_usec := Time.get_ticks_usec() - start_usec

	var result_bytes := rd.buffer_get_data(output_buffer)
	var candidates := _read_candidates(result_bytes)
	var elapsed_msec := float(elapsed_usec) / 1000.0
	var avg_usec := float(elapsed_usec) / float(TOTAL_VALUES)

	print("--- GoHash GPU compute test ---")
	print("shader: ", SHADER_PATH)
	print("target value: ", TARGET_VALUE)
	print("target hash16: ", target_hash16)
	print("total values: ", TOTAL_VALUES)
	print("workgroups x: ", groups_x)
	print("candidates: ", candidates.size())
	print("contains target: ", candidates.has(TARGET_VALUE))
	print("total time ms: ", elapsed_msec)
	print("average usec per value: ", avg_usec)

	assert(candidates.has(TARGET_VALUE))
	assert(candidates.size() >= 192 and candidates.size() <= 320)

	rd.free_rid(output_buffer)
	rd.free_rid(pipeline)
	rd.free_rid(uniform_set)
	rd.free_rid(shader)

func run_gpu_cpu_hash_match_test() -> void:
	print("--- GoHash GPU vs CPU hash16 match test ---")

	var shader_file := load(PREPARE_SHADER_PATH)
	if shader_file == null:
		push_error("No se pudo cargar el shader: %s" % PREPARE_SHADER_PATH)
		return

	var spirv: RDShaderSPIRV = shader_file.get_spirv()
	if spirv == null:
		push_error("El shader prepare-targets no devolvio SPIR-V.")
		return

	var rd := RenderingServer.create_local_rendering_device()
	if rd == null:
		push_error("RenderingDevice local no disponible para GPU vs CPU.")
		return

	var shader := rd.shader_create_from_spirv(spirv)
	if not shader.is_valid():
		push_error("Shader prepare-targets invalido o no compilado.")
		return

	var test_values := PackedInt32Array([
		0,
		1,
		2,
		3,
		255,
		256,
		65_535,
		65_536,
		837_498,
		TARGET_VALUE,
		TOTAL_VALUES - 1,
	])
	var count := test_values.size()
	var value_bytes := PackedByteArray()
	value_bytes.resize(count * 4)
	var hash_bytes := PackedByteArray()
	hash_bytes.resize(count * 4)
	hash_bytes.fill(0)

	for index in range(count):
		value_bytes.encode_u32(index * 4, test_values[index])

	var value_buffer := rd.storage_buffer_create(value_bytes.size(), value_bytes)
	var hash_buffer := rd.storage_buffer_create(hash_bytes.size(), hash_bytes)

	var value_uniform := RDUniform.new()
	value_uniform.uniform_type = RenderingDevice.UNIFORM_TYPE_STORAGE_BUFFER
	value_uniform.binding = 0
	value_uniform.add_id(value_buffer)

	var hash_uniform := RDUniform.new()
	hash_uniform.uniform_type = RenderingDevice.UNIFORM_TYPE_STORAGE_BUFFER
	hash_uniform.binding = 1
	hash_uniform.add_id(hash_buffer)

	var uniform_set := rd.uniform_set_create([value_uniform, hash_uniform], shader, 0)
	var pipeline := rd.compute_pipeline_create(shader)
	var push_constants := _make_prepare_push_constants(count)

	var start_usec := Time.get_ticks_usec()
	var compute_list := rd.compute_list_begin()
	rd.compute_list_bind_compute_pipeline(compute_list, pipeline)
	rd.compute_list_bind_uniform_set(compute_list, uniform_set, 0)
	rd.compute_list_set_push_constant(compute_list, push_constants, push_constants.size())
	rd.compute_list_dispatch(compute_list, 1, 1, 1)
	rd.compute_list_end()
	rd.submit()
	rd.sync()
	var elapsed_usec := Time.get_ticks_usec() - start_usec

	var result_bytes := rd.buffer_get_data(hash_buffer)
	var mismatches := 0

	for index in range(count):
		var value: int = test_values[index]
		var gpu_hash16: int = result_bytes.decode_u32(index * 4)
		var cpu_hash16: int = _cpu_siphash13_u32_hash16(value, SEED0_LOW, SEED0_HIGH, SEED1_LOW, SEED1_HIGH)
		var ok := gpu_hash16 == cpu_hash16
		print("value: ", value, " cpu hash16: ", cpu_hash16, " gpu hash16: ", gpu_hash16, " match: ", ok)
		if not ok:
			mismatches += 1

	print("values checked: ", count)
	print("mismatches: ", mismatches)
	print("total time usec: ", elapsed_usec)

	assert(mismatches == 0)

	rd.free_rid(uniform_set)
	rd.free_rid(value_buffer)
	rd.free_rid(hash_buffer)
	rd.free_rid(pipeline)
	rd.free_rid(shader)

func run_gpu_multi_target_test() -> void:
	print("--- GoHash GPU multi-target compute test ---")
	print("preparing random targets: ", MULTI_TARGET_COUNT)
	print("batch size: ", MULTI_TARGET_BATCH_SIZE)

	var prepare_shader_file := load(PREPARE_SHADER_PATH)
	if prepare_shader_file == null:
		push_error("No se pudo cargar el shader: %s" % PREPARE_SHADER_PATH)
		return

	var shader_file := load(MULTI_SHADER_PATH)
	if shader_file == null:
		push_error("No se pudo cargar el shader: %s" % MULTI_SHADER_PATH)
		return

	var prepare_spirv: RDShaderSPIRV = prepare_shader_file.get_spirv()
	if prepare_spirv == null:
		push_error("El shader prepare-targets no devolvio SPIR-V.")
		return

	var spirv: RDShaderSPIRV = shader_file.get_spirv()
	if spirv == null:
		push_error("El shader multi-target no devolvio SPIR-V.")
		return

	var rd := RenderingServer.create_local_rendering_device()
	if rd == null:
		push_error("RenderingDevice local no disponible para multi-target.")
		return

	var prepare_shader := rd.shader_create_from_spirv(prepare_spirv)
	if not prepare_shader.is_valid():
		push_error("Shader prepare-targets invalido o no compilado.")
		return

	var shader := rd.shader_create_from_spirv(spirv)
	if not shader.is_valid():
		push_error("Shader compute multi-target invalido o no compilado.")
		return

	var rng := RandomNumberGenerator.new()
	rng.seed = 0xC0FFEE

	var target_values := PackedInt32Array()

	for index in range(MULTI_TARGET_COUNT):
		var value: int = rng.randi_range(START_VALUE, START_VALUE + TOTAL_VALUES - 1)
		target_values.append(value)

	var prepare_pipeline := rd.compute_pipeline_create(prepare_shader)
	var pipeline := rd.compute_pipeline_create(shader)
	var groups_x := int(ceil(float(TOTAL_VALUES) / float(WORKGROUP_SIZE)))

	var start_usec := Time.get_ticks_usec()
	var total_matches := 0
	var found_count := 0
	var min_matches := 999999999
	var max_matches := 0
	var first_random_target_count := -1
	var batches := int(ceil(float(MULTI_TARGET_COUNT) / float(MULTI_TARGET_BATCH_SIZE)))

	for batch_index in range(batches):
		var batch_start := batch_index * MULTI_TARGET_BATCH_SIZE
		var batch_count: int = min(MULTI_TARGET_BATCH_SIZE, MULTI_TARGET_COUNT - batch_start)
		var target_hash_bytes := PackedByteArray()
		target_hash_bytes.resize(batch_count * 4)
		target_hash_bytes.fill(0)
		var target_value_bytes := PackedByteArray()
		target_value_bytes.resize(batch_count * 4)
		var counts_bytes := PackedByteArray()
		counts_bytes.resize(batch_count * 4)
		counts_bytes.fill(0)
		var found_bytes := PackedByteArray()
		found_bytes.resize(batch_count * 4)
		found_bytes.fill(0)

		for local_index in range(batch_count):
			var global_index := batch_start + local_index
			target_value_bytes.encode_u32(local_index * 4, target_values[global_index])

		var target_buffer := rd.storage_buffer_create(target_hash_bytes.size(), target_hash_bytes)
		var counts_buffer := rd.storage_buffer_create(counts_bytes.size(), counts_bytes)
		var found_buffer := rd.storage_buffer_create(found_bytes.size(), found_bytes)
		var target_value_buffer := rd.storage_buffer_create(target_value_bytes.size(), target_value_bytes)

		var prepare_target_value_uniform := RDUniform.new()
		prepare_target_value_uniform.uniform_type = RenderingDevice.UNIFORM_TYPE_STORAGE_BUFFER
		prepare_target_value_uniform.binding = 0
		prepare_target_value_uniform.add_id(target_value_buffer)

		var prepare_target_hash_uniform := RDUniform.new()
		prepare_target_hash_uniform.uniform_type = RenderingDevice.UNIFORM_TYPE_STORAGE_BUFFER
		prepare_target_hash_uniform.binding = 1
		prepare_target_hash_uniform.add_id(target_buffer)

		var prepare_uniform_set := rd.uniform_set_create([prepare_target_value_uniform, prepare_target_hash_uniform], prepare_shader, 0)
		var prepare_push_constants := _make_prepare_push_constants(batch_count)

		var target_uniform := RDUniform.new()
		target_uniform.uniform_type = RenderingDevice.UNIFORM_TYPE_STORAGE_BUFFER
		target_uniform.binding = 0
		target_uniform.add_id(target_buffer)

		var counts_uniform := RDUniform.new()
		counts_uniform.uniform_type = RenderingDevice.UNIFORM_TYPE_STORAGE_BUFFER
		counts_uniform.binding = 1
		counts_uniform.add_id(counts_buffer)

		var found_uniform := RDUniform.new()
		found_uniform.uniform_type = RenderingDevice.UNIFORM_TYPE_STORAGE_BUFFER
		found_uniform.binding = 2
		found_uniform.add_id(found_buffer)

		var target_value_uniform := RDUniform.new()
		target_value_uniform.uniform_type = RenderingDevice.UNIFORM_TYPE_STORAGE_BUFFER
		target_value_uniform.binding = 3
		target_value_uniform.add_id(target_value_buffer)

		var uniform_set := rd.uniform_set_create([target_uniform, counts_uniform, found_uniform, target_value_uniform], shader, 0)
		var push_constants := _make_multi_push_constants(batch_count)
		var batch_start_usec := Time.get_ticks_usec()
		var compute_list := rd.compute_list_begin()
		rd.compute_list_bind_compute_pipeline(compute_list, prepare_pipeline)
		rd.compute_list_bind_uniform_set(compute_list, prepare_uniform_set, 0)
		rd.compute_list_set_push_constant(compute_list, prepare_push_constants, prepare_push_constants.size())
		rd.compute_list_dispatch(compute_list, 1, 1, 1)
		rd.compute_list_add_barrier(compute_list)

		rd.compute_list_bind_compute_pipeline(compute_list, pipeline)
		rd.compute_list_bind_uniform_set(compute_list, uniform_set, 0)
		rd.compute_list_set_push_constant(compute_list, push_constants, push_constants.size())
		rd.compute_list_dispatch(compute_list, groups_x, batch_count, 1)
		rd.compute_list_end()
		rd.submit()
		rd.sync()
		var batch_elapsed_msec := float(Time.get_ticks_usec() - batch_start_usec) / 1000.0

		var count_result_bytes := rd.buffer_get_data(counts_buffer)
		var found_result_bytes := rd.buffer_get_data(found_buffer)

		for local_index in range(batch_count):
			var count: int = count_result_bytes.decode_u32(local_index * 4)
			var found: int = found_result_bytes.decode_u32(local_index * 4)
			total_matches += count
			min_matches = min(min_matches, count)
			max_matches = max(max_matches, count)
			if found == 1:
				found_count += 1
			if batch_index == 0 and local_index == 0:
				first_random_target_count = count

		print("batch ", batch_index + 1, "/", batches, " targets: ", batch_count, " time ms: ", batch_elapsed_msec)

		rd.free_rid(uniform_set)
		rd.free_rid(prepare_uniform_set)
		rd.free_rid(target_buffer)
		rd.free_rid(counts_buffer)
		rd.free_rid(found_buffer)
		rd.free_rid(target_value_buffer)

	var elapsed_usec := Time.get_ticks_usec() - start_usec
	var total_checks := TOTAL_VALUES * MULTI_TARGET_COUNT
	var elapsed_msec := float(elapsed_usec) / 1000.0
	var avg_usec_per_check := float(elapsed_usec) / float(total_checks)
	var avg_matches_per_target := float(total_matches) / float(MULTI_TARGET_COUNT)

	print("shader: ", MULTI_SHADER_PATH)
	print("target count: ", MULTI_TARGET_COUNT)
	print("batch size: ", MULTI_TARGET_BATCH_SIZE)
	print("total values per target: ", TOTAL_VALUES)
	print("total comparisons: ", total_checks)
	print("workgroups x: ", groups_x)
	print("workgroups y per batch: ", MULTI_TARGET_BATCH_SIZE)
	print("targets found: ", found_count)
	print("total matches: ", total_matches)
	print("average matches per target: ", avg_matches_per_target)
	print("min matches: ", min_matches)
	print("max matches: ", max_matches)
	print("total time ms: ", elapsed_msec)
	print("average usec per comparison: ", avg_usec_per_check)
	print("first random target value: ", target_values[0])
	print("first random target count: ", first_random_target_count)

	assert(found_count == MULTI_TARGET_COUNT)
	assert(avg_matches_per_target >= 192.0 and avg_matches_per_target <= 320.0)

	rd.free_rid(pipeline)
	rd.free_rid(shader)
	rd.free_rid(prepare_pipeline)
	rd.free_rid(prepare_shader)

func _make_push_constants(target_hash16: int) -> PackedByteArray:
	var bytes := PackedByteArray()
	bytes.resize(32)
	bytes.encode_u32(0, SEED0_LOW)
	bytes.encode_u32(4, SEED0_HIGH)
	bytes.encode_u32(8, SEED1_LOW)
	bytes.encode_u32(12, SEED1_HIGH)
	bytes.encode_u32(16, START_VALUE)
	bytes.encode_u32(20, target_hash16)
	bytes.encode_u32(24, TOTAL_VALUES)
	bytes.encode_u32(28, 0)
	return bytes

func _make_multi_push_constants(target_count: int) -> PackedByteArray:
	var bytes := PackedByteArray()
	bytes.resize(32)
	bytes.encode_u32(0, SEED0_LOW)
	bytes.encode_u32(4, SEED0_HIGH)
	bytes.encode_u32(8, SEED1_LOW)
	bytes.encode_u32(12, SEED1_HIGH)
	bytes.encode_u32(16, START_VALUE)
	bytes.encode_u32(20, TOTAL_VALUES)
	bytes.encode_u32(24, target_count)
	bytes.encode_u32(28, 0)
	return bytes

func _make_prepare_push_constants(target_count: int) -> PackedByteArray:
	var bytes := PackedByteArray()
	bytes.resize(32)
	bytes.encode_u32(0, SEED0_LOW)
	bytes.encode_u32(4, SEED0_HIGH)
	bytes.encode_u32(8, SEED1_LOW)
	bytes.encode_u32(12, SEED1_HIGH)
	bytes.encode_u32(16, target_count)
	bytes.encode_u32(20, 0)
	bytes.encode_u32(24, 0)
	bytes.encode_u32(28, 0)
	return bytes

func _read_candidates(bytes: PackedByteArray) -> PackedInt32Array:
	var candidates := PackedInt32Array()
	for offset in range(0, bytes.size(), 4):
		var value := bytes.decode_u32(offset)
		if value != NOT_MATCH:
			candidates.append(value)
	return candidates

func _cpu_siphash13_u32_hash16(value: int, seed0_low: int, seed0_high: int, seed1_low: int, seed1_high: int) -> int:
	var k0 := _u64(seed0_low, seed0_high)
	var k1 := _u64(seed1_low, seed1_high)
	var v0 := _xor64(k0, _u64(0x70736575, 0x736f6d65))
	var v1 := _xor64(k1, _u64(0x6e646f6d, 0x646f7261))
	var v2 := _xor64(k0, _u64(0x6e657261, 0x6c796765))
	var v3 := _xor64(k1, _u64(0x79746573, 0x74656462))
	var state := [v0, v1, v2, v3]
	var message := _u64(value, 0x04000000)

	state[3] = _xor64(state[3], message)
	_sip_round_state(state)
	state[0] = _xor64(state[0], message)
	state[2] = _xor64(state[2], _u64(0xff, 0))
	_sip_round_state(state)
	_sip_round_state(state)
	_sip_round_state(state)

	var hash64 := _xor64(_xor64(state[0], state[1]), _xor64(state[2], state[3]))
	return hash64["low"] & 0xffff

func _sip_round_state(state: Array) -> void:
	state[0] = _add64(state[0], state[1])
	state[2] = _add64(state[2], state[3])
	state[1] = _rotl64(state[1], 13)
	state[3] = _rotl64(state[3], 16)
	state[1] = _xor64(state[1], state[0])
	state[3] = _xor64(state[3], state[2])
	state[0] = _rotl64(state[0], 32)
	state[2] = _add64(state[2], state[1])
	state[0] = _add64(state[0], state[3])
	state[1] = _rotl64(state[1], 17)
	state[3] = _rotl64(state[3], 21)
	state[1] = _xor64(state[1], state[2])
	state[3] = _xor64(state[3], state[0])
	state[2] = _rotl64(state[2], 32)

func _u64(low: int, high: int) -> Dictionary:
	return {"low": low & 0xffffffff, "high": high & 0xffffffff}

func _add64(a: Dictionary, b: Dictionary) -> Dictionary:
	var a_low: int = int(a["low"])
	var a_high: int = int(a["high"])
	var b_low: int = int(b["low"])
	var b_high: int = int(b["high"])
	var low: int = (a_low + b_low) & 0xffffffff
	var carry: int = 1 if low < a_low else 0
	var high: int = (a_high + b_high + carry) & 0xffffffff
	return _u64(low, high)

func _xor64(a: Dictionary, b: Dictionary) -> Dictionary:
	return _u64(int(a["low"]) ^ int(b["low"]), int(a["high"]) ^ int(b["high"]))

func _rotl64(v: Dictionary, shift: int) -> Dictionary:
	shift &= 63
	var low: int = int(v["low"])
	var high: int = int(v["high"])
	if shift == 0:
		return _u64(low, high)
	if shift == 32:
		return _u64(high, low)
	if shift < 32:
		return _u64(
			((low << shift) | (high >> (32 - shift))) & 0xffffffff,
			((high << shift) | (low >> (32 - shift))) & 0xffffffff
		)

	shift -= 32
	return _u64(
		((high << shift) | (low >> (32 - shift))) & 0xffffffff,
		((low << shift) | (high >> (32 - shift))) & 0xffffffff
	)
