extends Control

# Instanciamos la clase Unarc expuesta desde Rust via GDExtension
var unarc = Unarc.new()

# Nodos UI
@onready var file_path_input = $MainLayout/Sidebar/PathSection/HBox/FilePathInput
@onready var password_input = $MainLayout/Sidebar/PasswordSection/PasswordInput
@onready var file_list = $MainLayout/Sidebar/FileList
@onready var status_label = $MainLayout/Sidebar/StatusLabel
@onready var progress_bar = $MainLayout/Sidebar/ProgressSection/ProgressBar
@onready var preview_panel = $MainLayout/PreviewArea/PreviewPanel
@onready var text_preview = $MainLayout/PreviewArea/PreviewPanel/ScrollContainer/TextPreview
@onready var image_preview = $MainLayout/PreviewArea/PreviewPanel/ImagePreview
@onready var hex_preview = $MainLayout/PreviewArea/PreviewPanel/HexPreview
@onready var preview_title = $MainLayout/PreviewArea/PanelHeader/HBoxContainer/PreviewTitle
@onready var extract_to_disk_btn = $MainLayout/PreviewArea/PanelHeader/HBoxContainer/ExtractToDiskButton

# Estados
var current_archive_bytes: PackedByteArray = PackedByteArray()
var current_archive_path: String = ""
var current_format: String = "zip"
var current_file_entries: Array = []
var active_mode: String = "none" # "ram", "disk", "progressive"
var selected_entry_metadata: Dictionary = {}
var detected_volumes: Array[String] = [] # Almacena todas las partes si es multi-volumen
var _multi_volume_fallback_flag: bool = false

func _ready() -> void:
	status_label.text = "Selecciona un archivo .7z, .rar o .zip. Carga desde Disco (Recomendado para archivos pesados >100MB) o desde RAM."
	preview_panel.hide()
	extract_to_disk_btn.hide()
	progress_bar.value = 0
	
	# Conectar señales de la lista y botones
	file_list.item_selected.connect(_on_file_item_selected)
	extract_to_disk_btn.pressed.connect(_on_extract_to_disk_pressed)

func _on_browse_button_pressed() -> void:
	$FileDialog.popup_centered()

func _on_file_dialog_file_selected(path: String) -> void:
	file_path_input.text = path
	status_label.text = "Archivo seleccionado: " + path.get_file()

# DETECTOR AUTOMÁTICO DE ARCHIVOS MULTI-VOLUMEN (EXPRESIVO)
# Si seleccionas "archivo.7z.001" o "archivo.part1.rar", escanea su carpeta para recolectar todas las partes ordenadamente
func _detect_multi_volumes(first_part_path: String) -> Array[String]:
	var volumes: Array[String] = []
	var global_first = ProjectSettings.globalize_path(first_part_path)
	var dir_path = global_first.get_base_dir()
	var file_name = global_first.get_file()
	
	var dir = DirAccess.open(dir_path)
	if not dir:
		return [global_first]
		
	var is_split = false
	var base_name = ""
	
	if file_name.ends_with(".001"):
		# Caso 7z.001 o zip.001
		base_name = file_name.left(file_name.length() - 4) # Remueve ".001"
		is_split = true
	elif file_name.to_lower().contains(".part1.") or file_name.to_lower().contains(".part01."):
		# Caso rar.part1.rar
		var part_idx = file_name.to_lower().find(".part")
		base_name = file_name.left(part_idx)
		is_split = true
		
	if not is_split:
		return [global_first]
		
	# Escanear el directorio para recolectar todos los volúmenes del split
	dir.list_dir_begin()
	var next_file = dir.get_next()
	while next_file != "":
		if not dir.current_is_dir():
			if next_file.begins_with(base_name):
				# Validar que sea extensión numérica (.001, .002) o contenga ".part"
				var next_ext = next_file.get_extension()
				if next_ext.is_valid_int() or next_file.to_lower().contains(".part"):
					var full_path = dir_path + "/" + next_file
					volumes.append(full_path)
		next_file = dir.get_next()
	dir.list_dir_end()
	
	# Ordenar los volúmenes de manera ascendente (.001, .002, etc.)
	volumes.sort()
	prints(volumes)
	return volumes

# MODO 1: CARGAR TODO EN RAM (Para archivos únicos pequeños)
func _on_load_in_ram_pressed() -> void:
	var path = file_path_input.text
	if path == "":
		status_label.text = "[Error] Especifica la ruta del archivo comprimido."
		return
		
	var global_path = ProjectSettings.globalize_path(path)
	if not FileAccess.file_exists(global_path):
		status_label.text = "[Error] El archivo no existe en el disco."
		return
		
	active_mode = "ram"
	current_archive_path = ""
	detected_volumes.clear()
	status_label.text = "Cargando archivo completo a la memoria RAM..."
	progress_bar.value = 25
	await get_tree().create_timer(0.05).timeout
	
	# Leer todo el archivo a RAM utilizando la API de Godot
	var file = FileAccess.open(global_path, FileAccess.READ)
	current_archive_bytes = file.get_buffer(file.get_length())
	file.close()
	
	current_format = path.get_extension().to_lower()
	progress_bar.value = 75
	status_label.text = "¡Cargado en RAM! (%s KB). Analizando en vivo..." % [current_archive_bytes.size() / 1024]
	await get_tree().create_timer(0.05).timeout
	
	_list_files_from_ram()

func _list_files_from_ram() -> void:
	file_list.clear()
	preview_panel.hide()
	extract_to_disk_btn.hide()
	selected_entry_metadata.clear()
	
	# Usamos la API de memoria de Rust correspondiente según haya contraseña o no
	var password = password_input.text
	if password != "":
		current_file_entries = unarc.get_entries_from_bytes_with_password(current_archive_bytes, current_format, password)
	else:
		current_file_entries = unarc.get_entries_from_bytes(current_archive_bytes, current_format)
	
	if current_file_entries.size() == 0:
		status_label.text = "[Error] No se encontraron entradas en RAM. ¿El archivo está encriptado?"
		progress_bar.value = 0
		return
		
	for entry in current_file_entries:
		var icon_text = "📁 " if entry["is_directory"] else "📄 "
		var size_text = "" if entry["is_directory"] else " (%s)" % _format_size(entry["size"])
		var item_idx = file_list.add_item("[RAM] " + icon_text + entry["name"] + size_text)
		file_list.set_item_metadata(item_idx, entry)
		
	progress_bar.value = 100
	status_label.text = "¡Modo RAM! %d archivos listados en vivo sin tocar disco." % current_file_entries.size()

# MODO 2: EXPLORACIÓN DIRECTA DESDE DISCO (Soporta archivos gigantescos y Multi-Volumen divididos)
func _on_load_from_disk_pressed() -> void:
	var path = file_path_input.text
	if path == "":
		status_label.text = "[Error] Especifica la ruta del archivo comprimido."
		return
		
	var global_path = ProjectSettings.globalize_path(path)
	if not FileAccess.file_exists(global_path):
		status_label.text = "[Error] El archivo no existe en el disco."
		return
		
	active_mode = "disk"
	current_archive_bytes.clear() # Liberamos la RAM para no saturar
	
	# Detectar automáticamente si es un archivo dividido / multi-volumen
	detected_volumes = _detect_multi_volumes(path)
	current_archive_path = detected_volumes[0]
	
	# Si es .7z.001 detectamos el formato original buscando el punto antes del split
	current_format = path.get_extension().to_lower()
	if current_format.is_valid_int() and path.to_lower().contains(".7z."):
		current_format = "7z"
	elif current_format.is_valid_int() and path.to_lower().contains(".zip."):
		current_format = "zip"
		
	status_label.text = "Explorando directo desde el disco (Cero consumo de RAM)..."
	progress_bar.value = 50
	await get_tree().create_timer(0.05).timeout
	
	file_list.clear()
	preview_panel.hide()
	extract_to_disk_btn.hide()
	selected_entry_metadata.clear()
	
	# Comprobar si el archivo está protegido por contraseña antes de intentar cargarlo
	var password = password_input.text
	var requires_password = false
	if detected_volumes.size() > 1:
		requires_password = unarc.is_archive_encrypted_multi_volume(detected_volumes, current_format)
	else:
		requires_password = unarc.is_archive_encrypted(current_archive_path)
		
	if requires_password and password == "":
		status_label.text = "[Advertencia] Este archivo requiere contraseña. Por favor, escribe la contraseña en el cuadro superior antes de cargar."
		progress_bar.value = 0
		return

	var detected_format = ""
	if unarc.is_supported_archive(current_archive_path):
		detected_format = unarc.get_archive_format_name(current_archive_path).to_lower()
		
	if detected_format != "" and detected_format != current_format:
		status_label.text = "¡Formato genérico detectado! Abriendo como: %s" % detected_format.to_upper()
		current_format = detected_format
	prints(detected_format , "formato automatico")
	prints(obtener_formato(detected_volumes) , "formato gdscript")
	# Listar entradas
	if detected_volumes.size() > 1:
		status_label.text = "Detectados %d volúmenes divididos en disco. Mapeando cabecera..." % detected_volumes.size()
		current_file_entries = unarc.get_entries_multi_volume_with_password(detected_volumes, obtener_formato(detected_volumes), password)
	else:
		# Archivo de volumen único convencional o genérico
		if password != "" and detected_format != "" and unarc.has_method("get_entries_with_format_and_password"):
			current_file_entries = unarc.get_entries_with_format_and_password(current_archive_path, detected_format, password)
		elif password != "":
			current_file_entries = unarc.get_entries_with_password(current_archive_path, password)
			if current_file_entries.size() == 0 and detected_format == "" and unarc.has_method("get_entries_with_format_and_password"):
				current_file_entries = unarc.get_entries_with_format_and_password(current_archive_path, "7z", password)
				if current_file_entries.size() > 0:
					current_format = "7z"
		elif detected_format != "" and unarc.has_method("get_entries_with_format"):
			current_file_entries = unarc.get_entries_with_format(current_archive_path, detected_format)
		else:
			current_file_entries = unarc.get_entries(current_archive_path)
			if current_file_entries.size() == 0 and unarc.has_method("get_entries_with_format"):
				current_file_entries = unarc.get_entries_with_format(current_archive_path, "7z")
				if current_file_entries.size() > 0:
					current_format = "7z"
	if current_file_entries.size() == 0:
		status_label.text = "[Error] No se pudieron leer cabeceras. ¿Contraseña incorrecta o faltan partes?"
		progress_bar.value = 0
		return
		
	for entry in current_file_entries:
		var prefix = "[Multi-Part] " if detected_volumes.size() > 1 else "[Disco] "
		var icon_text = "📁 " if entry["is_directory"] else "📄 "
		var size_text = "" if entry["is_directory"] else " (%s)" % _format_size(entry["size"])
		var item_idx = file_list.add_item(prefix + icon_text + entry["name"] + size_text)
		file_list.set_item_metadata(item_idx, entry)
		
	progress_bar.value = 100
	status_label.text = "¡Mapeado Exitoso! %d archivos listados desde la ruta física." % current_file_entries.size()

func _on_file_item_selected(index: int) -> void:
	var entry = file_list.get_item_metadata(index)
	if entry["is_directory"]:
		return
		
	selected_entry_metadata = entry
	var entry_name = entry["name"]
	var password = password_input.text
	var ext = entry_name.get_extension().to_lower()
	
	preview_title.text = "Visualizando: " + entry_name.get_file()
	extract_to_disk_btn.show()
	
	text_preview.hide()
	image_preview.hide()
	hex_preview.hide()
	preview_panel.show()
	
	var uncompressed_bytes = PackedByteArray()
	
	if active_mode == "ram" or active_mode == "progressive":
		status_label.text = "Descomprimiendo en RAM: " + entry_name.get_file()
		uncompressed_bytes = unarc.read_entry_bytes_from_bytes(
			current_archive_bytes,
			entry_name,
			current_format,
			password
		)
	else:
		# Desde disco
		if detected_volumes.size() > 1:
			# Para previsualizar un archivo ligero de un multi-volumen, lo volcamos temporalmente a memoria RAM
			status_label.text = "Extrayendo muestra en RAM desde Multi-Volumen..."
			# NOTA: En multi-volumen de disco, si es ligero, lo leemos extrayéndolo directamente a RAM temporal.
			# unarc-rs unifica la lectura directamente.
			uncompressed_bytes = unarc.read_entry_bytes_with_password(
				current_archive_path,
				entry_name,
				password
			)
		else:
			status_label.text = "Descomprimiendo en RAM desde Disco: " + entry_name.get_file()
			if password != "" and current_format in ["7z", "zip", "rar"] and unarc.has_method("read_entry_bytes_with_format_and_password"):
				uncompressed_bytes = unarc.read_entry_bytes_with_format_and_password(current_archive_path, entry_name, current_format, password)
			elif current_format in ["7z", "zip", "rar"] and unarc.has_method("read_entry_bytes_with_format"):
				uncompressed_bytes = unarc.read_entry_bytes_with_format(current_archive_path, entry_name, current_format)
			else:
				uncompressed_bytes = unarc.read_entry_bytes_with_password(
					current_archive_path,
					entry_name,
					password
				)
		
	if uncompressed_bytes.size() == 0:
		status_label.text = "[Error] No se pudo extraer la previsualización a RAM. ¿Contraseña incorrecta?"
		text_preview.show()
		text_preview.text = "Error al intentar extraer el archivo.\nSi es un archivo pesado (como un video), presiona 'Volcar a Disco' en la cabecera para transmitirlo por streaming."
		return
		
	status_label.text = "¡Extraído en RAM con éxito! Mostrando previsualización..."
	
	if ext in ["txt", "json", "xml", "ini", "cfg", "gd", "md"]:
		_show_text_preview(uncompressed_bytes)
	elif ext in ["png", "jpg", "jpeg", "webp"]:
		_show_image_preview(uncompressed_bytes, ext)
	else:
		_show_hex_preview(uncompressed_bytes)

func _show_text_preview(bytes: PackedByteArray) -> void:
	text_preview.show()
	var text_content = bytes.get_string_from_utf8()
	if text_content == "":
		text_content = bytes.get_string_from_ascii()
	text_preview.text = text_content

func _show_image_preview(bytes: PackedByteArray, ext: String) -> void:
	image_preview.show()
	var img = Image.new()
	var err = OK
	
	if ext == "png":
		err = img.load_png_from_buffer(bytes)
	elif ext in ["jpg", "jpeg"]:
		err = img.load_jpg_from_buffer(bytes)
	elif ext == "webp":
		err = img.load_webp_from_buffer(bytes)
		
	if err == OK:
		var tex = ImageTexture.create_from_image(img)
		image_preview.texture = tex
	else:
		text_preview.show()
		text_preview.text = "No se pudo decodificar la imagen en RAM."

func _show_hex_preview(bytes: PackedByteArray) -> void:
	hex_preview.show()
	var limit = min(bytes.size(), 256)
	var hex_string = "--- DUMP HEXADECIMAL DE PRIMEROS 256 BYTES (RAM) ---\n\n"
	
	var row = ""
	var chars = ""
	for i in range(limit):
		var byte = bytes[i]
		row += "%02X " % byte
		if byte >= 32 and byte <= 126:
			chars += char(byte)
		else:
			chars += "."
			
		if (i + 1) % 16 == 0 or i == limit - 1:
			if (i + 1) % 16 != 0:
				var missing = 16 - ((i + 1) % 16)
				for m in range(missing):
					row += "   "
			hex_string += "%04X | %s | %s\n" % [i - (i % 16), row, chars]
			row = ""
			chars = ""
			
	if bytes.size() > 256:
		hex_string += "\n... (%d bytes omitidos en la previsualización) ..." % [bytes.size() - 256]
		
	hex_preview.text = hex_string

# VOLCAR ARCHIVO PESADO A DISCO (Streaming nativo con 0 consumo de RAM)
func _on_extract_to_disk_pressed() -> void:
	if selected_entry_metadata.size() == 0:
		return
		
	var entry_name = selected_entry_metadata["name"]
	var output_file_path = "user://extracted_files/" + entry_name.get_file()
	var global_output_path = ProjectSettings.globalize_path(output_file_path)
	
	status_label.text = "Transmitiendo por streaming al disco: " + output_file_path + "..."
	progress_bar.value = 40
	await get_tree().create_timer(0.05).timeout
	
	var success = false
	var password = password_input.text
	
	if active_mode == "ram" or active_mode == "progressive":
		var bytes = unarc.read_entry_bytes_from_bytes(current_archive_bytes, entry_name, current_format, password)
		if bytes.size() > 0:
			var dir = output_file_path.get_base_dir()
			DirAccess.make_dir_recursive_absolute(dir)
			var file = FileAccess.open(output_file_path, FileAccess.WRITE)
			file.store_buffer(bytes)
			file.close()
			success = true
	else:
		# Modo Disco / Multi-volumen
		prints("DEBUG volumes:", detected_volumes.size(), detected_volumes)
		if detected_volumes.size() > 1:
			if unarc.has_method("extract_entry_multi_volume_with_password"):
				status_label.text = "Extrayendo desde Multi-Volumen con streaming directo a disco..."
				_multi_volume_fallback_flag = false
				success = unarc.extract_entry_multi_volume_with_password(detected_volumes, current_format, entry_name, global_output_path, password)
			else:
				_multi_volume_fallback_flag = true
				status_label.text = "[Warn] Streaming multi-volumen no disponible. Usando RAM como fallback..."
				var bytes = unarc.read_entry_bytes_with_password(current_archive_path, entry_name, password)
				if bytes.size() > 0:
					var dir = output_file_path.get_base_dir()
					DirAccess.make_dir_recursive_absolute(dir)
					var file = FileAccess.open(global_output_path, FileAccess.WRITE)
					file.store_buffer(bytes)
					file.close()
					success = true
		else:
			_multi_volume_fallback_flag = false
			if password != "":
				status_label.text = "Extrayendo archivo protegido con contraseña..."
				if current_format in ["7z", "zip", "rar"] and unarc.has_method("extract_entry_with_format_and_password"):
					success = unarc.extract_entry_with_format_and_password(current_archive_path, entry_name, global_output_path, current_format, password)
				else:
					var bytes = unarc.read_entry_bytes_with_password(current_archive_path, entry_name, password)
					if bytes.size() > 0:
						var dir = output_file_path.get_base_dir()
						DirAccess.make_dir_recursive_absolute(dir)
						var file = FileAccess.open(output_file_path, FileAccess.WRITE)
						file.store_buffer(bytes)
						file.close()
						success = true
			else:
				if unarc.is_supported_archive(current_archive_path):
					var detected_format = unarc.get_archive_format_name(current_archive_path).to_lower()
					if detected_format != "":
						success = unarc.extract_entry_with_format(current_archive_path, entry_name, global_output_path, detected_format)
					else:
						success = unarc.extract_entry(current_archive_path, entry_name, global_output_path)
				elif current_format in ["7z", "zip", "rar"]:
					success = unarc.extract_entry_with_format(current_archive_path, entry_name, global_output_path, current_format)
				else:
					success = unarc.extract_entry(current_archive_path, entry_name, global_output_path)
		
	if success:
		progress_bar.value = 100
		status_label.text = "¡Volcado de Archivo Exitoso! Guardado en: " + output_file_path
		text_preview.show()
		image_preview.hide()
		hex_preview.hide()
		if _multi_volume_fallback_flag:
			text_preview.text = "¡ARCHIVO VOLCADO AL DISCO CON ÉXITO! (vía RAM fallback)\n\n" + \
				"Ruta local: " + output_file_path + "\n" + \
				"Ruta global: " + global_output_path + "\n" + \
				"Tamaño unificado: " + _format_size(selected_entry_metadata["size"]) + "\n\n" + \
				"⚠️ AVISO: El streaming multi-volumen directo a disco aún no está implementado en la extensión Rust.\n" + \
				"Se usó un fallback que carga los bytes en RAM antes de escribirlos a disco.\n" + \
				"Para archivos muy grandes (>1GB) esto podría saturar la memoria."
		else:
			text_preview.text = "¡ARCHIVO VOLCADO AL DISCO POR STREAMING CON ÉXITO!\n\n" + \
				"Ruta local: " + output_file_path + "\n" + \
				"Ruta global: " + global_output_path + "\n" + \
				"Tamaño unificado: " + _format_size(selected_entry_metadata["size"]) + "\n\n" + \
				"Este método realiza streaming nativo directo, por lo que el archivo nunca saturó tu memoria RAM. ¡Perfecto para archivos pesados!"
	else:
		progress_bar.value = 0
		status_label.text = "[Error] No se pudo volcar el archivo al disco."

func _on_simulate_progressive_pressed() -> void:
	var path = file_path_input.text
	if path == "":
		status_label.text = "[Error] Selecciona un archivo real para simular la descarga en partes."
		return
		
	var global_path = ProjectSettings.globalize_path(path)
	if not FileAccess.file_exists(global_path):
		status_label.text = "[Error] El archivo no existe en el disco."
		return
		
	file_list.clear()
	preview_panel.hide()
	extract_to_disk_btn.hide()
	current_archive_bytes.clear()
	selected_entry_metadata.clear()
	detected_volumes.clear()
	active_mode = "progressive"
	
	var file = FileAccess.open(global_path, FileAccess.READ)
	var total_size = file.get_length()
	current_format = path.get_extension().to_lower()
	
	status_label.text = "Simulando descarga progresiva en RAM..."
	
	var chunk_size = ceil(total_size / 10.0)
	var downloaded_bytes = 0
	
	for i in range(1, 11):
		var bytes_chunk = file.get_buffer(chunk_size)
		current_archive_bytes.append_array(bytes_chunk)
		downloaded_bytes += bytes_chunk.size()
		
		var progress = int((float(downloaded_bytes) / total_size) * 100)
		progress_bar.value = progress
		
		status_label.text = "Descargando parte %d/10 en RAM: %d%% (%d/%d KB)" % [
			i, progress, downloaded_bytes / 1024, total_size / 1024
		]
		
		var password = password_input.text
		var temp_entries
		if password != "":
			temp_entries = unarc.get_entries_from_bytes_with_password(current_archive_bytes, current_format, password)
		else:
			temp_entries = unarc.get_entries_from_bytes(current_archive_bytes, current_format)
		if temp_entries.size() > 0 and file_list.get_item_count() == 0:
			status_label.text += " | ¡Cabecera descifrada en vivo en parte %d!" % i
			for entry in temp_entries:
				var icon_text = "📁 " if entry["is_directory"] else "📄 "
				var size_text = "" if entry["is_directory"] else " (%s)" % _format_size(entry["size"])
				var item_idx = file_list.add_item("[En vivo] " + icon_text + entry["name"] + size_text)
				file_list.set_item_metadata(item_idx, entry)
				
		await get_tree().create_timer(0.15).timeout
		
	file.close()
	status_label.text = "¡Simulación completada! Archivo 100% descargado y disponible en RAM."
	
	if file_list.get_item_count() == 0:
		_list_files_from_ram()

func _format_size(bytes: int) -> String:
	if bytes < 1024:
		return str(bytes) + " B"
	elif bytes < 1024 * 1024:
		return "%.1f KB" % [bytes / 1024.0]
	else:
		return "%.1f MB" % [bytes / (1024.0 * 1024.0)]

func _on_back_pressed() -> void:
	get_tree().change_scene_to_file("res://example_godot/unarc_test/example_unarc.tscn")


func obtener_formato(archivos: Array[String]) -> String:
	var permitidos = ["7z", "rar", "zip"]
	
	for ruta in archivos:
		var nombre = ruta.get_file()
		var ext = nombre.get_extension().to_lower()
		
		# Limpieza: Si es un número (001) o empieza con "part" (part1, part02)
		if ext.is_valid_int() or ext.begins_with("part"):
			nombre = nombre.get_basename()
			ext = nombre.get_extension().to_lower()
		
		if ext in permitidos:
			return ext
			
	return ""
