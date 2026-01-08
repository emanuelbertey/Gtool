extends Node

func _ready():
	test_cuckoo_filter()

func test_cuckoo_filter():
	print("--- Iniciando Test de Cuckoo Filter ---")
	
	# 1. Crear instancia
	var filter = CuckooFilterGodot.new()
	
	# 2. Inicializar (capacidad 1000, fingerprint 16 bits)
	filter.init_filter(10000, 16)
	print("Filtro inicializado.")

	# 3. Generar y agregar 100 hashes
	var hashes = []
	for i in range(100):
		var data = PackedByteArray()
		data.append_array(("dato_prueba_%d" % i).to_utf8_buffer())
		var h = filter.generate_hash(data)
		hashes.append(h)
		filter.add(h)
	
	print("100 hashes agregados.")

	# 4. Verificar que los 100 están presentes
	var total_presentes = 0
	for h in hashes:
		if filter.contains(h):
			total_presentes += 1
	
	print("Hashes verificados al inicio: %d/100" % total_presentes)

	# 5. Quitar 20 hashes
	var removidos = []
	for i in range(20):
		var h = hashes[i]
		prints("hashes : " , hashes[i])
		if filter.remove(h):
			removidos.append(h)
	
	print("20 hashes removidos.")

	# 6. Verificar que los 20 removidos YA NO ESTÁN y los otros 80 SIGUEN ESTANDO
	var errores_remocion = 0
	for h in removidos:
		if filter.contains(h):
			errores_remocion += 1
			
	var siguen_estando = 0
	for i in range(20, 100):
		if filter.contains(hashes[i]):
			siguen_estando += 1
			
	print("Errores (removidos que siguen apareciendo): %d" % errores_remocion)
	print("Hashes que siguen estando (deberian ser 80): %d" % siguen_estando)

	# 7. Probar Guardar y Cargar
	var path = "user://filtro_test.bin"
	if filter.save_to_file(ProjectSettings.globalize_path(path)):
		print("Filtro guardado en: ", path)
		
		# Crear un nuevo filtro y cargar
		var filter_nuevo = CuckooFilterGodot.new()
		if filter_nuevo.load_from_file(ProjectSettings.globalize_path(path)):
			print("Filtro cargado exitosamente.")
			if filter_nuevo.contains(hashes[50]):
				print("✅ Confirmado: El filtro cargado detecta el hash 50.")
			else:
				print("❌ Error: El filtro cargado no reconoce el hash 50.")
		else:
			print("❌ Error al cargar el filtro.")
	else:
		print("❌ Error al guardar el filtro.")

	print("--- Test Finalizado ---")
