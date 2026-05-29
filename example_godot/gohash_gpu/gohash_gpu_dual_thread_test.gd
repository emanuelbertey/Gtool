extends Node

const TOTAL_VALUES := 16_777_216
const WORKGROUP_SIZE := 256
const START_VALUE := 0
const SEED0_LOW := 0
const SEED0_HIGH := 0
const SEED1_LOW := 0
const SEED1_HIGH := 0
const TARGETS_PER_THREAD := 512
const TARGET_BATCH_SIZE := 16
const WORKER_COUNT := 2
const PREPARE_SHADER_PATH := "res://example_godot/gohash_gpu/siphash16_prepare_targets.glsl"
const MULTI_SHADER_PATH := "res://example_godot/gohash_gpu/siphash16_multi_targets.glsl"

var _threads: Array[Thread] = []
var _done: Array[bool] = []
var _results: Array = []
var _start_usec := 0

func _ready() -> void:
	print("--- GoHash GPU dual-thread multi-target test ---")
	print("workers: ", WORKER_COUNT)
	print("targets per worker: ", TARGETS_PER_THREAD)
	print("batch size: ", TARGET_BATCH_SIZE)
	print("values per target: ", TOTAL_VALUES)

	_start_usec = Time.get_ticks_usec()
	_threads.resize(WORKER_COUNT)
	_done.resize(WORKER_COUNT)
	_results.resize(WORKER_COUNT)

	for worker_id in range(WORKER_COUNT):
		_done[worker_id] = false
		var thread := Thread.new()
		_threads[worker_id] = thread
		var err := thread.start(Callable(self, "_thread_entry").bind(worker_id))
		if err != OK:
			push_error("No se pudo iniciar worker %d: %s" % [worker_id, err])
			_done[worker_id] = true

	set_process(true)

func _process(_delta: float) -> void:
	var all_done := true

	for worker_id in range(WORKER_COUNT):
		var thread := _threads[worker_id]
		if _done[worker_id] or thread == null:
			continue

		if thread.is_alive():
			all_done = false
			continue

		var result = thread.wait_to_finish()
		_results[worker_id] = result
		_done[worker_id] = true
		_print_worker_result(result)

	for done in _done:
		if not done:
			all_done = false

	if all_done:
		_print_total_result()
		set_process(false)

func _exit_tree() -> void:
	for worker_id in range(_threads.size()):
		var thread := _threads[worker_id]
		if thread != null and thread.is_started():
			thread.wait_to_finish()

func _thread_entry(worker_id: int) -> Dictionary:
	return _run_gpu_worker(worker_id)

func _run_gpu_worker(worker_id: int) -> Dictionary:
	var worker_start_usec := Time.get_ticks_usec()
	var prepare_shader_file := load(PREPARE_SHADER_PATH)
	var multi_shader_file := load(MULTI_SHADER_PATH)
	if prepare_shader_file == null or multi_shader_file == null:
		return _error_result(worker_id, "No se pudieron cargar shaders.")

	var prepare_spirv: RDShaderSPIRV = prepare_shader_file.get_spirv()
	var multi_spirv: RDShaderSPIRV = multi_shader_file.get_spirv()
	if prepare_spirv == null or multi_spirv == null:
		return _error_result(worker_id, "No se pudo obtener SPIR-V.")

	var rd := RenderingServer.create_local_rendering_device()
	if rd == null:
		return _error_result(worker_id, "RenderingDevice local no disponible.")

	var prepare_shader := rd.shader_create_from_spirv(prepare_spirv)
	var multi_shader := rd.shader_create_from_spirv(multi_spirv)
	if not prepare_shader.is_valid() or not multi_shader.is_valid():
		return _error_result(worker_id, "Shader invalido.")

	var prepare_pipeline := rd.compute_pipeline_create(prepare_shader)
	var multi_pipeline := rd.compute_pipeline_create(multi_shader)
	var groups_x := int(ceil(float(TOTAL_VALUES) / float(WORKGROUP_SIZE)))
	var batches := int(ceil(float(TARGETS_PER_THREAD) / float(TARGET_BATCH_SIZE)))
	var rng := RandomNumberGenerator.new()
	rng.seed = 0xC0FFEE + worker_id

	var target_values := PackedInt32Array()
	for _index in range(TARGETS_PER_THREAD):
		target_values.append(rng.randi_range(START_VALUE, START_VALUE + TOTAL_VALUES - 1))

	var total_matches := 0
	var found_count := 0
	var min_matches := 999999999
	var max_matches := 0
	var slowest_batch_msec := 0.0
	var fastest_batch_msec := 999999999.0

	for batch_index in range(batches):
		var batch_start := batch_index * TARGET_BATCH_SIZE
		var batch_count: int = min(TARGET_BATCH_SIZE, TARGETS_PER_THREAD - batch_start)
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

		var target_hash_buffer := rd.storage_buffer_create(target_hash_bytes.size(), target_hash_bytes)
		var target_value_buffer := rd.storage_buffer_create(target_value_bytes.size(), target_value_bytes)
		var counts_buffer := rd.storage_buffer_create(counts_bytes.size(), counts_bytes)
		var found_buffer := rd.storage_buffer_create(found_bytes.size(), found_bytes)

		var prepare_uniform_set := rd.uniform_set_create([
			_make_storage_uniform(0, target_value_buffer),
			_make_storage_uniform(1, target_hash_buffer),
		], prepare_shader, 0)

		var multi_uniform_set := rd.uniform_set_create([
			_make_storage_uniform(0, target_hash_buffer),
			_make_storage_uniform(1, counts_buffer),
			_make_storage_uniform(2, found_buffer),
			_make_storage_uniform(3, target_value_buffer),
		], multi_shader, 0)

		var prepare_push_constants := _make_prepare_push_constants(batch_count)
		var multi_push_constants := _make_multi_push_constants(batch_count)
		var batch_start_usec := Time.get_ticks_usec()
		var compute_list := rd.compute_list_begin()
		rd.compute_list_bind_compute_pipeline(compute_list, prepare_pipeline)
		rd.compute_list_bind_uniform_set(compute_list, prepare_uniform_set, 0)
		rd.compute_list_set_push_constant(compute_list, prepare_push_constants, prepare_push_constants.size())
		rd.compute_list_dispatch(compute_list, 1, 1, 1)
		rd.compute_list_add_barrier(compute_list)
		rd.compute_list_bind_compute_pipeline(compute_list, multi_pipeline)
		rd.compute_list_bind_uniform_set(compute_list, multi_uniform_set, 0)
		rd.compute_list_set_push_constant(compute_list, multi_push_constants, multi_push_constants.size())
		rd.compute_list_dispatch(compute_list, groups_x, batch_count, 1)
		rd.compute_list_end()
		rd.submit()
		rd.sync()
		var batch_msec := float(Time.get_ticks_usec() - batch_start_usec) / 1000.0
		slowest_batch_msec = max(slowest_batch_msec, batch_msec)
		fastest_batch_msec = min(fastest_batch_msec, batch_msec)

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

		rd.free_rid(multi_uniform_set)
		rd.free_rid(prepare_uniform_set)
		rd.free_rid(target_hash_buffer)
		rd.free_rid(target_value_buffer)
		rd.free_rid(counts_buffer)
		rd.free_rid(found_buffer)

	rd.free_rid(multi_pipeline)
	rd.free_rid(prepare_pipeline)
	rd.free_rid(multi_shader)
	rd.free_rid(prepare_shader)

	var elapsed_usec := Time.get_ticks_usec() - worker_start_usec
	var total_checks := TOTAL_VALUES * TARGETS_PER_THREAD
	return {
		"worker_id": worker_id,
		"ok": found_count == TARGETS_PER_THREAD,
		"targets": TARGETS_PER_THREAD,
		"found": found_count,
		"total_matches": total_matches,
		"average_matches": float(total_matches) / float(TARGETS_PER_THREAD),
		"min_matches": min_matches,
		"max_matches": max_matches,
		"total_checks": total_checks,
		"elapsed_msec": float(elapsed_usec) / 1000.0,
		"average_usec_per_comparison": float(elapsed_usec) / float(total_checks),
		"fastest_batch_msec": fastest_batch_msec,
		"slowest_batch_msec": slowest_batch_msec,
	}

func _error_result(worker_id: int, message: String) -> Dictionary:
	return {
		"worker_id": worker_id,
		"ok": false,
		"error": message,
	}

func _print_worker_result(result: Dictionary) -> void:
	print("--- worker ", result.get("worker_id", -1), " done ---")
	if result.has("error"):
		print("error: ", result["error"])
		return

	print("ok: ", result["ok"])
	print("targets: ", result["targets"])
	print("targets found: ", result["found"])
	print("total matches: ", result["total_matches"])
	print("average matches per target: ", result["average_matches"])
	print("min matches: ", result["min_matches"])
	print("max matches: ", result["max_matches"])
	print("total comparisons: ", result["total_checks"])
	print("elapsed ms: ", result["elapsed_msec"])
	print("average usec per comparison: ", result["average_usec_per_comparison"])
	print("fastest batch ms: ", result["fastest_batch_msec"])
	print("slowest batch ms: ", result["slowest_batch_msec"])

func _print_total_result() -> void:
	var elapsed_msec := float(Time.get_ticks_usec() - _start_usec) / 1000.0
	var total_targets := 0
	var total_found := 0
	var total_matches := 0
	var total_checks := 0
	var ok := true

	for result in _results:
		if typeof(result) != TYPE_DICTIONARY:
			ok = false
			continue
		if result.has("error"):
			ok = false
			continue
		ok = ok and bool(result["ok"])
		total_targets += int(result["targets"])
		total_found += int(result["found"])
		total_matches += int(result["total_matches"])
		total_checks += int(result["total_checks"])

	print("--- dual-thread total ---")
	print("ok: ", ok)
	print("workers: ", WORKER_COUNT)
	print("total targets: ", total_targets)
	print("total found: ", total_found)
	print("total matches: ", total_matches)
	print("average matches per target: ", float(total_matches) / float(max(1, total_targets)))
	print("total comparisons: ", total_checks)
	print("wall time ms: ", elapsed_msec)
	print("wall average usec per comparison: ", float(Time.get_ticks_usec() - _start_usec) / float(max(1, total_checks)))

func _make_storage_uniform(binding: int, rid: RID) -> RDUniform:
	var uniform := RDUniform.new()
	uniform.uniform_type = RenderingDevice.UNIFORM_TYPE_STORAGE_BUFFER
	uniform.binding = binding
	uniform.add_id(rid)
	return uniform

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
