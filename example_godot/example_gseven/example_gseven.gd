extends Control

var gseven = Gseven.new()

const EXTRACT_DIR = "user://gseven_extracted"

func _ready():
	$Label.text = "Listo. Selecciona un archivo .7z para probar Gseven."

func _on_test_pressed() -> void:
	$Label.text = "Iniciando prueba...\n"

	var path = $PathInput.text
	if path == "":
		$Label.text += "Error: Introduce una ruta de archivo .7z\n"
		return

	var global = ProjectSettings.globalize_path(path)
	if not FileAccess.file_exists(global):
		$Label.text += "Error: El archivo no existe: %s\n" % global
		return

	var pass = $PassInput.text

	var absolute_extract_dir = ProjectSettings.globalize_path(EXTRACT_DIR)

	# 1. Listar entradas
	$Label.text += "\n1. Leyendo entradas con Gseven:\n"
	var entries
	if pass == "":
		entries = gseven.get_entries(global)
	else:
		entries = gseven.get_entries_with_password(global, pass)

	if entries.size() == 0:
		$Label.text += "   No se encontraron entradas (contraseña incorrecta?)\n"
		return

	for entry in entries:
		var info = "   - %s | %d bytes" % [entry["name"], entry["size"]]
		if entry["is_directory"]: info += " [DIR]"
		$Label.text += info + "\n"

	# 2. Leer primera entrada en memoria
	$Label.text += "\n2. Leyendo primera entrada en memoria:\n"
	var first_name = entries[0]["name"]
	if not entries[0]["is_directory"]:
		var bytes
		if pass == "":
			bytes = gseven.extract_entry_to_buffer(global, first_name)
		else:
			bytes = gseven.extract_entry_to_buffer(global, first_name, pass)
		if bytes.size() > 0:
			var preview = bytes.get_string_from_utf8()
			$Label.text += "   '%s' -> %d bytes leidos\n" % [first_name, bytes.size()]
			if preview.length() < 200:
				$Label.text += "   Contenido: '%s'\n" % preview.strip_edges()
		else:
			$Label.text += "   Error leyendo '%s'\n" % first_name

	# 3. Extraer todo
	$Label.text += "\n3. Extrayendo todo a:\n   %s\n" % absolute_extract_dir
	var success
	if pass == "":
		success = gseven.extract_all(global, absolute_extract_dir)
	else:
		success = gseven.extract_all(global, absolute_extract_dir, pass)
	if success:
		$Label.text += "   Extraccion completa exitosa!\n"
	else:
		$Label.text += "   Error en extraccion\n"

	# 4. Extraer solo la primera entrada a disco
	if not entries[0]["is_directory"]:
		$Label.text += "\n4. Extrayendo solo '%s':\n" % first_name
		var single_dest = absolute_extract_dir + "/_single_" + first_name
		var ok
		if pass == "":
			ok = gseven.extract_entry(global, first_name, single_dest)
		else:
			ok = gseven.extract_entry(global, first_name, single_dest, pass)
		if ok:
			$Label.text += "   Extraida a: %s\n" % single_dest
		else:
			$Label.text += "   Error extrayendo entrada individual\n"

func _on_file_selected(path: String) -> void:
	$PathInput.text = path
	$FileDialog.visible = false

func _on_browse_pressed() -> void:
	$FileDialog.popup_centered()
