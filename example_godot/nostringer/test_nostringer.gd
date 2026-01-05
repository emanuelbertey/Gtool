extends Control

var nostringer
var message
var ring_pubkeys


var kp1 
var kp2 
var kp3 


var blsag_sig
var blsag_res
var blsag_ki_sign



func _ready():
	test_nostringer()

func test_nostringer():
	nostringer = Nostringer.new()
	print("--- Starting Nostringer Test (TEXT MODE) ---")

	# 1. Setup
	kp1 = nostringer.generate_keypair("xonly")
	kp2 = nostringer.generate_keypair("compressed")
	kp3 = nostringer.generate_keypair("xonly")

	ring_pubkeys = [
		kp1["public_key"],
		kp2["public_key"], 
		kp3["public_key"]
	]

	message = "This is a secret message to the group.".to_utf8_buffer()
	$TextEdit.text = message.get_string_from_utf8()
	# 3. Firmar SAG - AÑADIDO 5º ARGUMENTO ""
	print("\n--- Testing SAG Signature ---")
	var sag_res = nostringer.sign(message, kp2["private_key"], ring_pubkeys, "sag")
	var sag_sig = sag_res["signature"]
	print("SAG Signature length: ", sag_sig.length())

	# 4. Verificar
	var sag_verify = nostringer.verify(sag_sig, message, ring_pubkeys)
	print("SAG Signature valid: ", sag_verify.get("valid", false))
	assert(sag_verify.get("valid", false), "SAG verification failed!")

	# 5. Firmar BLSAG - AÑADIDO 5º ARGUMENTO ""
	print("\n--- Testing BLSAG Signature ---")
	blsag_res = nostringer.sign(message, kp2["private_key"], ring_pubkeys, "blsag")
	blsag_sig = blsag_res["signature"]
	blsag_ki_sign = blsag_res["key_image"]
	print("Key Image (from sign): ", blsag_ki_sign)

	# 6. Verificar BLSAG
	var blsag_verify = nostringer.verify(blsag_sig, message, ring_pubkeys)
	var blsag_valid = blsag_verify.get("valid", false)
	var blsag_ki_rec = blsag_verify.get("key_image", "")
	
	print("BLSAG Signature valid: ", blsag_valid)
	print("Key Image (recovered): ", blsag_ki_rec)
	assert(blsag_valid, "BLSAG verification failed!")
	assert(blsag_ki_sign == blsag_ki_rec, "Key Image mismatch!")

	print("\n--- Nostringer TEXT Test Completed ---")


	var message_bad = "This is a bad menssage.".to_utf8_buffer()
	blsag_verify = nostringer.verify(blsag_sig, message_bad, ring_pubkeys)
	blsag_valid = blsag_verify.get("valid", false)
	blsag_ki_rec = blsag_verify.get("key_image", "")
	
	

func _on_sag_or_blsag_toggled(toggled_on: bool) -> void:
	prints(toggled_on)
	if $"sag or blsag".text == "Sag ":
		$"sag or blsag".text = "BLsag"
	else:
		$"sag or blsag".text = "Sag "
	pass # Replace with function body.
	
	



func _on_quit_x_pressed() -> void:
	self.queue_free()
	pass # Replace with function body.


func _on_return_img_pressed() -> void:
	var blsag_verify = nostringer.verify(blsag_sig, $TextEdit.text.to_utf8_buffer(), ring_pubkeys)
	var blsag_valid = blsag_verify.get("valid", false)
	var blsag_ki_rec = blsag_verify.get("key_image", "")
	if blsag_valid:
	#print("BLSAG Signature valid: ", blsag_valid)
		print("Key Image (recovered): ", blsag_ki_rec)
		$info.text += "\n Key Image (recovered): " + str(blsag_ki_rec)
		$info.text += "\n Key Image (global): " + str(blsag_ki_sign)
	
		return
	prints(blsag_valid, "BLSAG verification failed!")
	$info.text += "\n BLSAG " + str(blsag_valid) + " verification!"
	
	pass # Replace with function body.


func _on_add_ids_pressed() -> void:
	kp2 = nostringer.generate_keypair("xonly")

	ring_pubkeys = [
		kp1["public_key"],
		kp2["public_key"], 
		kp3["public_key"]
	]
	pass # Replace with function body.


func _on_comprobar_pressed() -> void:
	var blsag_verify = nostringer.verify(blsag_sig, $TextEdit.text.to_utf8_buffer(), ring_pubkeys)
	var blsag_valid = blsag_verify.get("valid", false)
	var blsag_ki_rec = blsag_verify.get("key_image", "")
	#print("BLSAG Signature valid: ", blsag_valid)
	print("Key Image (recovered): ", blsag_ki_rec)
	#prints(blsag_valid, "BLSAG verification failed!")
	$info.text += "\n BLSAG : " + str(blsag_valid) + " verification!"
	if blsag_ki_sign == blsag_ki_rec:
		$info.text += "\n BLSAG image mach "
		
	pass # Replace with function body.


func _on_firmar_pressed() -> void:
	# 5. Firmar BLSAG - AÑADIDO 5º ARGUMENTO ""
	print("\n--- Testing BLSAG Signature ---")
	blsag_res = nostringer.sign($TextEdit.text.to_utf8_buffer(), kp2["private_key"], ring_pubkeys, "blsag")
	blsag_sig = blsag_res["signature"]
	blsag_ki_sign = blsag_res["key_image"]
	print("Key Image (from sign): ", blsag_ki_sign)
	pass # Replace with function body.


func _on_origin_pressed() -> void:
	$TextEdit.text = message.get_string_from_utf8()
	pass # Replace with function body.
