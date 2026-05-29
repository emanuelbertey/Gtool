extends Node

const TOTAL_VALUES := 16_777_216
const TARGET_VALUE := 1_234_567
const SEED0 := 0
const SEED1 := 0

func _ready() -> void:
	run_gohash_test()
	run_10_arrays_test()

func run_gohash_test() -> void:
	var gohash := GoHash.new()
	var data := PackedByteArray()
	data.append_array(("numero_%d" % TARGET_VALUE).to_utf8_buffer())
	var hash64 := gohash.generate_hash_seeded(data, SEED0, SEED1)
	var hash16 := gohash.set_from_hash(hash64)
	var single_value_ok := gohash.contains_bytes_seeded(data, SEED0, SEED1)
	var start_usec := Time.get_ticks_usec()
	var matches := gohash.count_matches(0, TOTAL_VALUES)
	var elapsed_usec := Time.get_ticks_usec() - start_usec
	var elapsed_msec := float(elapsed_usec) / 1000.0
	var avg_usec_per_hash := float(elapsed_usec) / float(TOTAL_VALUES)
	var expected := gohash.expected_matches(TOTAL_VALUES)

	print("--- GoHash 16-bit test ---")
	print("target value: ", TARGET_VALUE)
	print("seed0: ", SEED0)
	print("seed1: ", SEED1)
	print("target hash64: ", hash64)
	print("target hash16: ", hash16)
	print("total values: ", TOTAL_VALUES)
	print("storage bits: ", gohash.storage_bits())
	print("storage bytes: ", gohash.storage_bytes())
	print("hash space: ", gohash.hash_space())
	print("expected candidates: ", expected)
	print("real candidates in same hash: ", matches)
	print("single data contains: ", single_value_ok)
	print("16M total time ms: ", elapsed_msec)
	print("average usec per hash: ", avg_usec_per_hash)

	assert(gohash.contains_hash(hash64))
	assert(gohash.contains_hash16(hash16))
	assert(gohash.contains_bytes_seeded(data, SEED0, SEED1))
	assert(single_value_ok)
	assert(matches >= 192 and matches <= 320)
	print("OK: 16,777,216 valores dan cerca de 256 candidatos para un hash de 16 bits.")

func run_10_arrays_test() -> void:
	print("--- GoHash 10 arrays x 16M test ---")

	var total_start_usec := Time.get_ticks_usec()
	var total_matches := 0

	for index in range(10):
		var gohash := GoHash.new()
		var data := PackedByteArray()
		data.append_array(("array_%02d_valor_%d" % [index, TARGET_VALUE + index]).to_utf8_buffer())

		var hash_start_usec := Time.get_ticks_usec()
		var hash64 := gohash.generate_hash_seeded(data, SEED0, SEED1)
		var hash16 := gohash.set_from_hash(hash64)
		var hash_elapsed_usec := Time.get_ticks_usec() - hash_start_usec

		var single_value_ok := gohash.contains_bytes_seeded(data, SEED0, SEED1)

		var scan_start_usec := Time.get_ticks_usec()
		var matches := gohash.count_matches(0, TOTAL_VALUES)
		var scan_elapsed_usec := Time.get_ticks_usec() - scan_start_usec
		var scan_elapsed_msec := float(scan_elapsed_usec) / 1000.0
		var avg_usec_per_hash := float(scan_elapsed_usec) / float(TOTAL_VALUES)
		total_matches += matches

		print("array index: ", index)
		print("  hash64: ", hash64)
		print("  hash16: ", hash16)
		print("  single data contains: ", single_value_ok)
		print("  hash create usec: ", hash_elapsed_usec)
		print("  16M scan time ms: ", scan_elapsed_msec)
		print("  average usec per hash: ", avg_usec_per_hash)
		print("  matches: ", matches)

		assert(single_value_ok)
		assert(gohash.contains_hash(hash64))
		assert(gohash.contains_hash16(hash16))
		assert(matches >= 192 and matches <= 320)

	var total_elapsed_usec := Time.get_ticks_usec() - total_start_usec
	var total_elapsed_msec := float(total_elapsed_usec) / 1000.0
	var total_checks := TOTAL_VALUES * 10
	var total_avg_usec := float(total_elapsed_usec) / float(total_checks)

	print("10 arrays total matches: ", total_matches)
	print("10 arrays total time ms: ", total_elapsed_msec)
	print("10 arrays average usec per hash: ", total_avg_usec)
	print("OK: 10 arrays verificados individualmente contra 16,777,216 valores cada uno.")
