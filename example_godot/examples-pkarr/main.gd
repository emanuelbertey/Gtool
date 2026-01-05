extends Control
var pkarr = preload("res://example_godot/examples-pkarr/pkarr.tscn")
var key_nostr = preload("res://example_godot/examples-pkarr/key.tscn")
var data = {}
#const SAVE_DIR = "res://saves/"
var platform
var save_name = "save.key"
var save_path# = SAVE_DIR 
var local = "res://"

func _ready() -> void:
	
	
	#region dir 
	
	platform = OS.get_name()

	if platform == "Android":
		local = "user://"
		print("Estamos en Android")

	dir_contents(local)




	
	
	var peer = Gpkarr.new()

	#prints(peer.get_secret_bytes())
	#peer.info_ips()
	prints("creando key ", peer.key_rand())

	var key = [
		199, 133, 251, 69, 66, 206, 61, 213, 151, 163, 166, 14, 142, 46, 94, 231,
		66, 126, 8, 67, 114, 56, 186, 37, 12, 18, 111, 207, 0, 223, 229, 145,
	]

	var packed_key = PackedByteArray()
	for byte in key:
		packed_key.append(byte)

	prints("public key",peer.public_key(packed_key))
	#prints(peer.prepare_packet("emagorgrod","fffrood",peer.key_rand()))

func dir_contents(path):
	var dir = DirAccess.open(path)
	if dir:
		dir.list_dir_begin()
		var file_name = dir.get_next()
		while file_name != "":
			if dir.current_is_dir():
				print("Found directory: " + file_name)
			else:
				#print("Found file: " + file_name)
				#print("file estencion: " + file_name.get_extension())
				if file_name.get_extension() == ".key" : prints("estencion .key")
			file_name = dir.get_next()
	else:
		print("An error occurred when trying to access the path.")







func _on_pkarr_pressed() -> void:
	#pkarr.instantiate()
	#add_child(pkarr)
	
	var menu = preload("res://example_godot/examples-pkarr/pkarr.tscn").instantiate()
	add_child(menu)
	prints("instancio escena")
	await get_tree().create_timer(3).timeout
	pass # Replace with function body.



func _on_key_nostr_pressed() -> void:
	add_child(key_nostr.instantiate())
	pass # Replace with function body.




func load_data():
	if $LineEdit.text == "":
		prints("error no ingreso pass")
	save_path = local + save_name
	var file = FileAccess.open_encrypted_with_pass(save_path, FileAccess.READ, $LineEdit.text)
	if file.file_exists(save_path):
		var error = file.get_open_error()
		if error == OK:
			var data = file.get_var()
			self.data = data
			prints(data)
			file.close()
			prints(data)

func save_data():
	if $LineEdit.text == "":
		prints("error no ingreso pass")
	save_path = local + save_name
	var file = FileAccess.open_encrypted_with_pass(save_path, FileAccess.WRITE,$LineEdit.text)
	var error = file.get_open_error()
	if error == OK:
		#if file.file_exists(save_path):
			#prints("el archivo existe editalo o eliminalo primero ")
			#file.close()
			#return
		file.store_var(data)
		prints(file.get_sha256(save_path))
		file.close()
	prints(data.size())
	prints("data saved")




#func _on_save_key_pressed() -> void:
	##if key_user.text == "":
		##prints("se olvido de ingresar una key")
		##return
	#if data.has(key_user.text):
		#console_write(str("la key " ,key_user.text,"  existe pruebe otra key"))
		#prints("la key " ,key_user.text,"  existe  pruebe otra key")
		#return
	#if value_user.text == "":
		#console_write("se olvido de ingresar un valor")
		#return
	#data[key_user.text] = value_user.text
	#pass 


func _on_crear_pressed() -> void:
	data = {
		"empty" : "v.1.0.0",
		"pin" : 3,
		"pkarr" : 6,
		"nostr" : 4,
		"webrtc" : 11,
	}
	save_data()
	pass # Replace with function body.


func _on_button_pressed() -> void:
	self.queue_free()
	pass # Replace with function body.
