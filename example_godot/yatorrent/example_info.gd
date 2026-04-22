extends Node

var info_torrent = InfoTorrent.new()
var peer = TPeer.new()
var peerip
var peerport
var inx = 1
var data
func _ready() -> void:
	# 1. Ejemplo desde un Magnet/InfoHash (Asíncrono con Red)
	#test_magnet("magnet:?xt=urn:btih:01c137287d6f0ed05a56742dae794f632c79ff3d")
	
	# 2. Ejemplo desde archivo .torrent (Síncrono/Local)
	# test_file("C:/Users/Emabe/Downloads/ubuntu-24.04.torrent")
	pass
func _process(_delta: float) -> void:
	# IMPORTANTE: Necesario para recibir la metadata cuando se usa fetch_metadata()
	# poll_metadata() revisa si los hilos de fondo terminaron y emite la señal
	if info_torrent.poll_metadata():
		print("--- Metadata Poll: Éxito ---")

func test_magnet(uri: String):
	info_torrent = InfoTorrent.new()
	
	# Conectamos la señal para saber cuando la metadata esté lista
	info_torrent.metadata_loaded.connect(_on_metadata_ready.bind(info_torrent))
	
	if info_torrent.load_from_magnet(uri):
		print("--- Magnet Cargado ---")
		
		# OPCIONAL: Puedes añadir trackers personalizados desde GDScript (HTTPS soportado)
		#info_torrent.add_tracker("https://tracker.nanoset.net:443/announce")
		# info_torrent.set_trackers(["udp://...", "https://..."])
		
		print("Hash: ", info_torrent.get_info_hash())
		print("Nombre (pre-metadata): ", info_torrent.get_display_name())
		print("Iniciando búsqueda en red (DHT/Trackers)...")
		
		info_torrent.fetch_metadata() # Arranca hilos de Rust para buscar metadatos
	else:
		print("Error en formato de Magnet/Hash")

func test_file(path: String):
	info_torrent = InfoTorrent.new()
	if info_torrent.load_from_file(path):
		print("\n✅ Archivo .torrent cargado localmente:")
		print("Nombre: ", info_torrent.get_display_name())
		print("Hash: ", info_torrent.get_info_hash())
		print("Tamaño total: ", info_torrent.get_total_size())
		
		var files = info_torrent.get_files()
		for f in files:
			print("- ", f.path, " (", String.humanize_size(f.size), ")")
	else:
		print("Error cargando archivo: ", path)

func _on_metadata_ready(info: InfoTorrent):
	print("\n✅ Metadata recibida desde la red!")
	print("Nombre Real: ", info.get_display_name())
	print("Archivos encontrados:")
	var files = info.get_files()
	for f in files:
		print("- ", f.path, " (", String.humanize_size(f.size), ")")
	
	print("Tamaño Total: ", String.humanize_size(info.get_total_size()))

func _on_button_pressed() -> void:
	if info_torrent.get_info_hash() == "":
		prints("valor nulo")
		return
	print("Total: ", info_torrent.get_total_size())
	print("Nombre: ", info_torrent.get_display_name())
	print("Hash: ", info_torrent.get_info_hash())
	print("All Piece: ", info_torrent.get_piece_count())
	

	var indice_a_pedir = 0 
	var hash_bruto = get_hash_de_pieza(indice_a_pedir)
	var hash_hex = bytes_a_hex(hash_bruto)
	var ha =  info_torrent.get_piece_hashes()
	print("Hash Piece ", indice_a_pedir, " (Hex): ", hash_hex)
	print("Hash Piece ", indice_a_pedir, " (Raw): ", hash_bruto)
	print("Hash All : ", ha.size() / 20, " (Raw Size): ", hash_bruto.size())
	print("Tamaño total: ", info_torrent.get_total_size())
	prints("tamaño del piece " , info_torrent.get_piece_length())
	prints("piece custom ",get_piece_size(0))
	var files = info_torrent.get_files()
	for f in files:
		print("- ", f.path, " (", String.humanize_size(f.size), ")")

# hash de 20 bytes de una pieza 
func get_hash_de_pieza(indice: int) -> PackedByteArray:
	var todos_los_hashes = info_torrent.get_piece_hashes()
	var inicio = indice * 20
	return todos_los_hashes.slice(inicio, inicio + 20)

# Convierte de Bytes a Hexadecimal (SHA1 format)
func bytes_a_hex(bytes: PackedByteArray) -> String:
	var hex = ""
	for b in bytes:
		hex += "%02x" % b
	return hex


func get_piece_size(idx) -> int:
	if idx == null:
		push_warning("get_piece_size: El índice es Nil, se usará 0 por defecto.")
		idx = 0
		
	if not info_torrent:
		return 0
		
	var total_size = info_torrent.get_total_size()
	var piece_length = info_torrent.get_piece_length()
	var piece_count = info_torrent.get_piece_count()
	
	if idx < 0 or idx >= piece_count:
		return 0
	
	if idx == piece_count - 1:
		var remainder = total_size % piece_length
		return remainder if remainder > 0 else piece_length
		
	return piece_length


func _on_uri_pressed() -> void:
	test_magnet("magnet:?xt=urn:btih:01c137287d6f0ed05a56742dae794f632c79ff3d")
	pass # Replace with function body.


func _on_torrent_pressed() -> void:
	test_file("C:/Users/Emabe/Downloads/ubuntu-25.10-desktop-amd64.iso.torrent")
	pass # Replace with function body.


func _on_piece_pressed() -> void:
	var piece_idx = int(inx)
	data = peer.request_piece_raw(
		info_torrent.get_info_hash(),
		str($VBoxContainer/ip.text),
		int($VBoxContainer/port.text),
		piece_idx,
		get_piece_size(piece_idx),  # tamaño REAL de esta pieza (última puede ser más chica)
		int(info_torrent.get_total_size()),    # tamaño TOTAL del torrent
		info_torrent.get_piece_hash(piece_idx)
	)
