extends Control

func _ready():
	test_binary_modes()

func test_binary_modes():
	var nostringer = Nostringer.new()
	print("--- Iniciando Prueba BINARIA Multimodal ---")

	# Setup 
	var kp = nostringer.generate_keypair("xonly")
	var kp_dummy = nostringer.generate_keypair("xonly")
	var ring = [kp["public_key"], kp_dummy["public_key"]]
	var message = "Mensaje secreto".to_utf8_buffer()

	# 1. Modo SAG - USAMOS 4 ARGUMENTOS
	print("\n[MODO SAG]")
	var res_sag = nostringer.sign_bin(message, kp["private_key"], ring, "sag")
	var sag_sig = res_sag["signature"]
	
	if sag_sig.size() > 0:
		var sag_res_v = nostringer.verify_bin(sag_sig, message, ring)
		print("Verificación SAG Binaria: ", sag_res_v.get("valid", false))

	# 2. Modo BLSAG - USAMOS 4 ARGUMENTOS
	print("\n[MODO BLSAG]")
	var res_blsag = nostringer.sign_bin(message, kp["private_key"], ring, "blsag")
	var blsag_sig = res_blsag["signature"]
	var blsag_ki_sign = res_blsag["key_image"]
	
	if blsag_sig.size() > 0:
		var blsag_res_v = nostringer.verify_bin(blsag_sig, message, ring)
		var blsag_ok = blsag_res_v.get("valid", false)
		var blsag_ki_rec = blsag_res_v.get("key_image", PackedByteArray())
		
		print("Verificación BLSAG Binaria: ", blsag_ok)
		assert(blsag_ok)
		assert(blsag_ki_sign == blsag_ki_rec, "Key Image Binaria mismatch")

	print("\n--- ¡Pruebas Binarias Finalizadas! ---")
