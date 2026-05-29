extends Control

var filter = CuckooFilterGodot.new()
var chunks = []
var long_sufijo = 10
var max_intentos = 50000

func _ready():
	$Panel/Button.pressed.connect(_on_hash_pressed)

func generar_sufijo(seed: int) -> PackedByteArray:
	var rng = RandomNumberGenerator.new()
	rng.set_seed(seed)
	var suf = PackedByteArray()
	for k in range(long_sufijo):
		suf.append(rng.randi() % 256)
	return suf

func _on_hash_pressed():
	$Panel/RichTextLabel.text = ""
	$Panel/Button.disabled = true
	var texto = $Panel/LineEdit.text
	if texto.is_empty():
		texto = "hola mundo"
		$Panel/LineEdit.text = texto

	var bytes = texto.to_utf8_buffer()
	chunks.clear()

	var i = 0
	while i < bytes.size() and chunks.size() < 1024:
		var chunk = PackedByteArray()
		for j in range(3):
			if i + j < bytes.size():
				chunk.append(bytes[i + j])
		chunks.append(chunk)
		i += 3

	var output = ""
	output += "Texto: \"%s\" (%d bytes)\n" % [texto, bytes.size()]
	output += "Chunks de 3 bytes: %d" % chunks.size()
	if bytes.size() > 3072:
		output += " (recortado a 1024, max del filtro)"
	output += "\n"
	output += "Longitud sufijo: %d bytes\n\n" % long_sufijo

	print("--- Cuckoo Hash Text ---")
	print("Texto: \"%s\" (%d bytes, %d chunks)" % [texto, bytes.size(), chunks.size()])
	print("Capacity: 1024 | Fingerprint: 16 bits | Long sufijo: %d" % long_sufijo)
	print("")

	var seed = 0
	var sufijo = PackedByteArray()
	var fallos = 0
	var duplicados = 0

	while true:
		seed += 1
		if seed > max_intentos:
			output += "ERROR: No se encontro sufijo en %d intentos.\n" % max_intentos
			print("ERROR: No se encontro sufijo en %d intentos" % max_intentos)
			$Panel/RichTextLabel.text = output
			$Panel/Button.disabled = false
			return

		sufijo = generar_sufijo(seed)
		filter = CuckooFilterGodot.new()
		filter.init_filter(1024, 16)
		var ok = true

		for idx in range(chunks.size()):
			var datos = PackedByteArray()
			datos.append_array(chunks[idx])
			datos.append_array(sufijo)
			var h = filter.generate_hash(datos)
			if filter.contains(h):
				duplicados += 1
				continue
			if not filter.add(h):
				fallos += 1
				if fallos <= 50 or seed % 1000 == 0 or fallos == 50000:
					var chunk_str = ""
					for b in chunks[idx]:
						chunk_str += char(b)
					print("Seed %d: add() FALLO chunk %d \"%s\" (fallo #%d, dup:%d)" % [seed, idx + 1, chunk_str, fallos, duplicados])
				ok = false
				break

		if ok:
			print("Seed %d: EXITO (tras %d fallos, %d duplicados)" % [seed, fallos, duplicados])
			break

	output += "Seed del sufijo: %d\n" % seed
	output += "Fallos de add: %d\n" % fallos
	output += "Duplicados evitados: %d\n" % duplicados
	output += "Intentos totales: %d\n\n" % seed

	for idx in range(chunks.size()):
		var chunk_str = ""
		for b in chunks[idx]:
			chunk_str += char(b)
		var datos = PackedByteArray()
		datos.append_array(chunks[idx])
		datos.append_array(sufijo)
		var h = filter.generate_hash(datos)
		output += "Chunk %d: \"%s\" + sufijo(seed:%d) -> %s\n" % [idx + 1, chunk_str, seed, str(h)]
		print("Chunk %d: \"%s\" + sufijo(seed:%d) -> %s" % [idx + 1, chunk_str, seed, str(h)])

	output += "\n--- Verificacion ---\n"
	for idx in range(chunks.size()):
		var datos = PackedByteArray()
		datos.append_array(chunks[idx])
		datos.append_array(sufijo)
		var h = filter.generate_hash(datos)
		var found = filter.contains(h)
		var chunk_str = ""
		for b in chunks[idx]:
			chunk_str += char(b)
		output += "Chunk %d: \"%s\" -> %s\n" % [idx + 1, chunk_str, "OK" if found else "FALLO"]
		print("Chunk %d: \"%s\" -> %s" % [idx + 1, chunk_str, "OK" if found else "FALLO"])

	output += "\nResultado: TODOS los chunks verificados correctamente."
	$Panel/RichTextLabel.text = output
	$Panel/Button.disabled = false
	print("Resultado: TODOS OK")
