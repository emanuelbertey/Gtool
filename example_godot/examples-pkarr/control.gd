extends Control

@onready var peer: Gpkarr = $Peerinfo
var time_ttl:int = 30
var keysy = []
var keys = []
var search = false
var counter = 0

func _ready() -> void:
	
	prints("test compare")
	var str1 = "pkarr_and_godot"
	var str2 = "pkarr_and_rust"
	var mach = is_mach(str1 , str2 , 5)
	prints("test compare result : ", mach)
	
	
	keysy= peer.seed_to_key("pkarr_and_godot")
	prints("llave de semilla ")
	prints(keysy)
	prints("******************************************")
	
	
	peer.log_to_file("hola",OS.get_user_data_dir() + "/log.txt")
	var pub =$CodeEdit.text #"s7wmbfk7c17eqw4sgncftsd3p4adoqiajixbia5q6c4ywgtawd3y"#"tkucztxt7xhrb41miqb7xknhijngpgy7tyhete15ycok7naaonsy"
	if pub.length() < 32:
		pub = peer.public_key(keysy)
		#pub = "tkucztxt7xhrb41miqb7xknhijngpgy7tyhete15ycok7naaonsy"
		$CodeEdit.text = pub
	pass
	#var peer = Peerinfo.new()

func _process(delta: float) -> void:
	if search:
		sear()
		if counter % 10000 == 0:
			prints("paso 10k : " , counter)
		pass
	pass

func _on_peerinfo_resolv(data: String) -> void:
	prints("desde la señal de godot : " , data)
	$Label.text = txt_linea(str(data))
	pass # Replace with function body.


func _on_button_pressed() -> void:
	var key = [
		199, 133, 251, 69, 66, 206, 61, 213, 151, 163, 166, 14, 142, 46, 94, 231,
		66, 126, 8, 67, 114, 56, 186, 37, 12, 18, 111, 207, 0, 223, 229, 145,
	]
	var packed_key = PackedByteArray()
	for byte in key:
		packed_key.append(byte)
	#var key = "o4dksfbqk85ogzdb5osziw6befigbuxmuxkuxq8434q89uj56uyy"
	var mode = $mode.text #"dht"#relays   dht
	var pub =$CodeEdit.text
	#var relays = PackedStringArray()# vacío si no usás relays
	var relays := PackedStringArray([
	"https://relay.pkarr.org",
	"https://pkarr.pubky.org"
	#"http://127.0.0.1:6881/"
])
	if pub.length() < 32:
		pub = peer.public_key(packed_key)
	prints(pub)
	var res = peer.resolve_key(pub,mode,relays)
	if res:
		$Label.text = txt_linea(res)
		$publicar.text = "echo resolver "
	else:
		prints("error")
		$publicar.text = "error resolver  "
	
	pass # Replace with function body.


func _on_button_2_pressed() -> void:
	var mode = $mode.text #"dht"#relays   dht
	var relays := PackedStringArray([
	"https://relay.pkarr.org",
	"https://pkarr.pubky.org"
	#"http://127.0.0.1:6881/"
])
	var key
	if keys.size() < 32:
		key = [
		199, 133, 251, 69, 66, 206, 61, 213, 151, 163, 166, 14, 142, 46, 94, 231,
		66, 126, 8, 67, 114, 56, 186, 37, 12, 18, 111, 207, 0, 223, 229, 145,]
	else:
		key = keys
	
	var packed_key = PackedByteArray()
	for byte in key:
		packed_key.append(byte)
	
	#if peer.prepare_packet(str($clave.text),str($valor.text),time_ttl,mode , relays,packed_key):
		#$publicar.text = "publicado "
	#else:
		#prints("error")
		#$publicar.text = "error publicado "
	#
	peer.ttl = 30
	if peer.prepare_packet(str($clave.text),str($valor.text),mode , relays,packed_key ):
		$publicar.text = "publicado "
	else:
		prints("error")
		$publicar.text = "error publicado "
	
	

func txt_linea(linea: String) -> String:
	var regex = RegEx.new()
	regex.compile(r'(\w+)\.[^\s]+\s+IN\s+\d+\s+TXT\s+"([^"]+)"')

	var match = regex.search(linea)
	if match == null:
		return ""

	var nombre = match.get_string(1)    
	var contenido = '"' + match.get_string(2) + '"' 

	return nombre + " " + contenido


func _on_exit_pressed() -> void:
	self.queue_free()
	pass # Replace with function body.


func _on_crea_key_pressed() -> void:
	search = true
	return
	keys = peer.key_rand()
	var key_is = false
	$key_vault.crear_pressed()
	$CodeEdit.text = peer.public_key(keys)
	if $keygen.text != "":
		for i in range(100):
			if is_mach(str(peer.public_key(keys)),$keygen.text,4):
				key_is = true
				return
		if key_is:
			$CodeEdit.text = str(keys)
		else:
			prints("no mach ")
	#prints("creando key ", keys , "sizekey : " , keys.size())
	pass # Replace with function body.
func sear():
	#keys = peer.key_rand()
	for i in range(100):
		keys = peer.key_rand()

		counter += 1
		var keygen = str(peer.public_key(keys))
		if keygen.begins_with($keygen.text):#is_mach(str(keygen),$keygen.text,$keygen.text.length()):
			prints(keygen)
			prints(keys)
			#counter += 1
			search = false
			return
	

func _on_save_key_pressed() -> void:
	prints("aqui se guarda")
	$key_vault.save_data()
	pass # Replace with function body.


func _on_time_ttl_value_changed(value: float) -> void:
	time_ttl = int(value)
	$count_timer.text = str(value)
	prints("cargo ttl : " , time_ttl)
	pass # Replace with function body.


func _on_load_key_pressed() -> void:
	prints("aqui se carga")
	$key_vault.load_data()
	pass # Replace with function body.

func is_mach(cadena: String, comparacion: String, cantidad: int) -> bool:
	# .left(cantidad) toma los primeros n caracteres
	# Comparamos si el inicio de la primera cadena es igual a la segunda
	return cadena.left(cantidad) == comparacion.left(cantidad)
