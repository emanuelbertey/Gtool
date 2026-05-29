extends Control

var filter = CuckooFilterGodot.new()
var repetido = 8

func _ready():
	$Button.pressed.connect(_test)

func _test():
	$RichTextLabel.text = ""
	$Button.disabled = true

	filter = CuckooFilterGodot.new()
	filter.init_filter(1024, 16)

	var fallos_add = 0
	var total_add = 0
	var hashes = []

	print("--- Test Bug: 1024 hashes x %d repetidos ---" % repetido)
	print("")

	for i in range(1024):
		var datos = PackedByteArray()
		datos.append_array(("dato_%d" % i).to_utf8_buffer())
		var h = filter.generate_hash(datos)
		hashes.append(h)

		for r in range(repetido):
			total_add += 1
			if not filter.add(h):
				fallos_add += 1
				if fallos_add <= 10:
					print("FALLO add() en hash %d (i:%d, rep:%d)" % [h, i, r])

	print("")
	print("Total add(): %d" % total_add)
	print("Fallos add(): %d" % fallos_add)

	var output = ""
	output += "Capacidad: 1024 | Fingerprint: 16 bits\n"
	output += "Repeticiones por hash: %d\n\n" % repetido
	output += "Total add(): %d\n" % total_add
	output += "Fallos add(): %d\n\n" % fallos_add

	output += "--- Verificacion ---\n"
	var fallos_verify = 0
	for i in range(1024):
		var datos = PackedByteArray()
		datos.append_array(("dato_%d" % i).to_utf8_buffer())
		var h = filter.generate_hash(datos)
		if not filter.contains(h):
			fallos_verify += 1
			if fallos_verify <= 10:
				print("FALLO contains() en hash %d (i:%d)" % [h, i])
			output += "Hash %d (i:%d) -> FALLO\n" % [h, i]

	output += "\nFallos en verify: %d / 1024\n" % fallos_verify
	output += "Tasa de error: %.2f%%\n" % [float(fallos_verify) / 1024.0 * 100.0]

	if fallos_verify == 0 and fallos_add == 0:
		output += "\nResultado: SIN ERRORES"
	else:
		output += "\nResultado: ERRORES DETECTADOS"

	$RichTextLabel.text = output
	$Button.disabled = false

	print("Fallos verify: %d / 1024" % fallos_verify)
	print("Resultado: %s" % ("SIN ERRORES" if (fallos_verify == 0 and fallos_add == 0) else "ERRORES"))
