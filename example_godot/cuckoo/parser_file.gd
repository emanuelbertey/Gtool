extends Control

var filter = CuckooFilterGodot.new()
var hashes = []

func _ready() -> void:
	# 1. Crear instancia e inicializar
	filter = CuckooFilterGodot.new()
	filter.init_filter(1024, 16)
	print("Filtro inicializado.")

	# 2. Agregar un hash de prueba inicial 16777215
	var data_init = ("dat0%d" % 5).to_utf8_buffer()
	var h_init = filter.generate_hash(data_init)
	hashes.append(h_init)
	filter.add(h_init)
	
	print("Iniciando ciclo de 16,777,216 iteraciones...")
	
	# 3. Iniciar el Timer (Medición)
	var tiempo_inicio = Time.get_ticks_usec()
	
	# 4. Bucle principal
	var total_ciclos = 16777216
	var coincidencias = 0
	
	for i in range(total_ciclos):
		var dat = ("dat0%d" % i).to_utf8_buffer()
		var h = filter.generate_hash(dat)
		
		if filter.contains(h):
			prints("Elemento encontrado en ciclo:", i)
			coincidencias += 1
		
		# Feedback cada 2 millones de ciclos para no bloquear la consola
		if i % 2000000 == 0:
			print("Progreso: %d%%" % [(i * 100) / total_ciclos])
			
	# 5. Finalizar Timer
	var tiempo_fin = Time.get_ticks_usec()
	var tiempo_total_us = tiempo_fin - tiempo_inicio
	var tiempo_total_s = tiempo_total_us / 1000000.0
	
	# 6. Reporte final
	print("--- Resultados del Benchmark ---")
	print("Ciclos totales: ", total_ciclos)
	print("Tiempo total: ", tiempo_total_s, " segundos")
	print("Promedio por ciclo: ", (tiempo_total_us / float(total_ciclos)), " us (microsegundos)")
	print("Coincidencias encontradas: ", coincidencias)
	print("1500 hashes agregados (proceso finalizado).")

func _process(delta: float) -> void:
	pass
