extends Control

var filter = CuckooFilterGodot.new()
var chunks = []
var long_sufijo = 4
var tokens_aleatorios: Array = []
func _ready():
	_test()
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
	var texto = "".join(tokens_aleatorios)#$Panel/LineEdit.text
	if texto.is_empty():
		texto = "hola mundo"
		$Panel/LineEdit.text = texto

	var bytes = texto.to_utf8_buffer()
	if bytes.size() > 3072:
		bytes = bytes.slice(0, 3072)
		texto = bytes.get_string_from_utf8()
		$Panel/LineEdit.text = texto

	chunks.clear()

	var i = 0
	while i < bytes.size():
		var chunk = PackedByteArray()
		for j in range(3):
			if i + j < bytes.size():
				chunk.append(bytes[i + j])
		chunks.append(chunk)
		i += 3

	var output = ""
	output += "Texto: \"%s\" (%d bytes)\n" % [texto, bytes.size()]
	output += "Chunks de 3 bytes: %d (max 1024)\n" % chunks.size()
	output += "Longitud sufijo: %d bytes\n" % long_sufijo
	output += "Datos hasheados: chunk + posicion + sufijo\n\n"

	print("--- Cuckoo Hash Text con Posicion ---")
	print("Texto: \"%s\" (%d bytes, %d chunks)" % [texto, bytes.size(), chunks.size()])
	print("Long sufijo: %d | Datos: chunk + pos + suf" % long_sufijo)
	print("")

	var seed = 1
	var sufijo = PackedByteArray()
	var fallos = 0

	while true:
		seed += 1
		if seed > 10000:
			output += "ERROR: No se encontro sufijo en 50000 intentos."
			print("ERROR: No se encontro sufijo en 50000 intentos")
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
			datos.append_array([idx])
			datos.append_array(sufijo)
			var h = filter.generate_hash(datos)
			if not filter.add(h):
				fallos += 1
				if fallos <= 10 or seed % 500 == 0:
					var chunk_str = ""
					for b in chunks[idx]:
						chunk_str += char(b)
					print("Seed %d: add() FALLO chunk %d \"%s\" (fallo #%d)" % [seed, idx + 1, chunk_str, fallos])
				ok = false
				break

		if ok:
			print("Seed %d: EXITO (tras %d fallos)" % [seed, fallos])
			break

	output += "Seed del sufijo: %d\n" % seed
	output += "Fallos de add: %d\n" % fallos
	output += "Intentos totales: %d\n\n" % seed

	for idx in range(chunks.size()):
		var chunk_str = ""
		for b in chunks[idx]:
			chunk_str += char(b)
		var datos = PackedByteArray()
		datos.append_array(chunks[idx])
		datos.append_array([idx])
		datos.append_array(sufijo)
		var h = filter.generate_hash(datos)
		output += "Chunk %d: \"%s\" + pos + sufijo(seed:%d) -> %s\n" % [idx + 1, chunk_str, seed, str(h)]
		#print("Chunk %d: \"%s\" + pos + sufijo(seed:%d) -> %s" % [idx + 1, chunk_str, seed, str(h)])
		#prints(chunks[idx])
		#prints([idx & 0xFF, (idx >> 8) & 0xFF])
	output += "\n--- Verificacion ---\n"
	var all_ok = true
	for idx in range(chunks.size()):
		var datos = PackedByteArray()
		datos.append_array(chunks[idx])
		datos.append_array([idx])
		datos.append_array(sufijo)
		var h = filter.generate_hash(datos)
		var found = filter.contains(h)
		var chunk_str = ""
		for b in chunks[idx]:
			chunk_str += char(b)
		output += "Chunk %d: \"%s\" -> %s\n" % [idx + 1, chunk_str, "OK" if found else "FALLO"]
		#print("Chunk %d: \"%s\" -> %s" % [idx + 1, chunk_str, "OK" if found else "FALLO"])
		if not found:
			all_ok = false

	output += "\nResultado: "
	output += "TODOS OK" if all_ok else "ERROR"
	$Panel/RichTextLabel.text = output
	$Panel/Button.disabled = false
	print("Resultado: %s" % ("TODOS OK" if all_ok else "ERROR"))
	
	_desc(1,filter,seed)
	





func _test() -> void:
	# Inicializamos la semilla del generador aleatorio de Godot
	randomize()
	
	# Generamos los 1000 tokens de 3 bytes
	tokens_aleatorios = generar_texto_aleatorio(1000, 3)
	
	# Imprimimos los primeros para verificar y el total
	print("--- PRIMEROS 10 TOKENS GENERADOS (ENTROPÍA PURA) ---")
	for i in range(10):
		print("Token %d: \"%s\"" % [i, tokens_aleatorios[i]])
		
	print("-----------------------------------------------------")
	print("Total de tokens generados: ", tokens_aleatorios.size())
	print("Tamaño total en bytes crudos: ", tokens_aleatorios.size() * 3)


func generar_texto_aleatorio(cantidad_tokens: int, tamaño_token: int) -> Array:
	var lista_tokens: Array = []
	
	# Rango ASCII imprimible estándar (desde el espacio ' ' (32) hasta la '~' (126))
	var ascii_min: int = 32
	var ascii_max: int = 126
	
	for i in range(cantidad_tokens):
		var token: String = ""
		for j in range(tamaño_token):
			# randi_range es seguro y genera una distribución uniforme
			var caracter_ascii: int = randi_range(ascii_min, ascii_max)
			token += char(caracter_ascii)
		lista_tokens.append(token)
		
	return lista_tokens



#var filter = CuckooFilterGodot.new()
var hashes = []
func _desc(idx: int, filter_instance, seed_usada: int) -> void:
	print("Iniciando ciclo de 16,777,216 iteraciones...")
	
	# 1. Iniciar el Timer
	var tiempo_inicio = Time.get_ticks_usec()
	
	# 2. Variables de control
	var total_ciclos = 16777216
	var coincidencias = 0
	var sufijo = generar_sufijo(seed_usada)
	
	# Pre-alojamos el buffer con el tamaño físico exacto en memoria:
	# 3 bytes (token) + 1 byte (idx) + 4 bytes (sufijo) = 8 bytes fijos.
	var datos = PackedByteArray()
	datos.resize(8)
	
	# Inyectamos la posición fija y el sufijo en sus posiciones exactas de memoria fuera del bucle
	datos[3] = idx & 0xFF  # Forzamos que sea un único byte puro
	datos[4] = sufijo[0]
	datos[5] = sufijo[1]
	datos[6] = sufijo[2]
	datos[7] = sufijo[3]
	
	# 3. Bucle principal de alta velocidad
	for i in range(total_ciclos):
		# Descomposición matemática del índice dinámico 'i'
		var byte1 = (i >> 16) & 0xFF
		var byte2 = (i >> 8) & 0xFF
		var byte3 = i & 0xFF

		# Inyectamos los 3 bytes dinámicos directo en el frente del buffer
		datos[0] = byte1
		datos[1] = byte2
		datos[2] = byte3
		
		# Generamos el hash con la estructura limpia de 8 bytes nativos
		var h = filter_instance.generate_hash(datos)
		
		# Verificación en el Filtro Cuckoo
		if filter_instance.contains(h):
			# Reconstrucción cosmética rápida para el log
			var chunk_str = ""
			for b in [byte1, byte2, byte3]:
				if b >= 32 and b <= 126:
					chunk_str += char(b)
				else:
					chunk_str += "."
			
			prints("¡MATCH DIRECTO ENCONTRADO!", i)
			prints("Bytes puros:", [byte1, byte2, byte3])
			prints("Texto:", chunk_str)
			coincidencias += 1
		#if filter_instance.contains(h):
	# LA COMPARACIÓN QUE ME PEDISTE:
	# Comparamos si el array de bytes del ciclo [byte1, byte2, byte3] 
	# es IGUAL al contenido real que tiene guardado chunks[idx]
			if chunks[idx] == PackedByteArray([byte1, byte2, byte3]):
				prints("-> ¡MATCH REAL CONFIRMADO EN LA VUELTA MÁGICA!", i)
				#coincidencias += 1  
				#return
		#
		# Control de progreso
		if i % 4000000 == 0 and i > 0:
			print("Progreso: %d%%" % [(i * 100) / total_ciclos])
			
	# 4. Reporte final
	var tiempo_fin = Time.get_ticks_usec()
	var tiempo_total_s = (tiempo_fin - tiempo_inicio) / 1000000.0
	
	print("--- Resultados del Benchmark ---")
	print("Ciclos totales: ", total_ciclos)
	print("Tiempo total: ", tiempo_total_s, " segundos")
	print("Coincidencias encontradas: ", coincidencias)
	print("Proceso finalizado.")
