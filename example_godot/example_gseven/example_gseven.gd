extends Control

var gseven = Gseven.new()

const EXTRACT_DIR = "user://gseven_extracted"

func _ready():
	$Label.text = "Selecciona un .7z y presiona Listar Entradas"

func _on_list_pressed() -> void:
	var path = $PathInput.text
	if path == "":
		$Label.text = "Error: Introduce una ruta"
		return

	var global = ProjectSettings.globalize_path(path)
	if not FileAccess.file_exists(global):
		$Label.text = "Error: No existe: %s" % global
		return

	var passg = $PassInput.text
	var is_multi = global.ends_with(".001")
	var paths_arr: Array[String]
	if is_multi:
		paths_arr = _gather_volumes(global)
		if paths_arr.is_empty():
			$Label.text = "Error: No se encontraron volumenes"
			return

	var entries
	if is_multi:
		if passg == "":
			entries = gseven.get_entries_multi_volume(paths_arr)
		else:
			entries = gseven.get_entries_multi_volume_with_password(paths_arr, passg)
	else:
		if passg == "":
			entries = gseven.get_entries(global)
		else:
			entries = gseven.get_entries_with_password(global, passg)

	if entries.size() == 0:
		$Label.text = "No se encontraron entradas (pass incorrecta?)"
		return

	var total_size = 0
	var text = "Entradas (%d):\n\n" % entries.size()
	for i in entries.size():
		var e = entries[i]
		var name = e["name"]
		var size = e["size"]
		var compressed = e.get("compressed_size", 0)
		var dir_mark = " [DIR]" if e["is_directory"] else ""
		total_size += size
		text += "%d: %s  (%s -> %s)%s\n" % [i, name, _fmt(compressed), _fmt(size), dir_mark]

	text += "\nTotal descomprimido: %s" % _fmt(total_size)
	$Label.text = text

	set_meta("entries", entries)
	set_meta("paths_arr", paths_arr)
	set_meta("is_multi", is_multi)
	set_meta("global_path", global)

func _parse_indices() -> Array[int]:
	var entries = get_meta("entries", [])
	if entries.is_empty():
		return []

	var raw = $EntrySelect.text.strip_edges()
	if raw == "all":
		var all: Array[int] = []
		for i in entries.size():
			all.append(i)
		return all

	var out: Array[int] = []
	for part in raw.split(","):
		var t = part.strip_edges()
		if t.is_valid_int():
			var idx = t.to_int()
			if idx >= 0 and idx < entries.size():
				out.append(idx)
	return out

func _get_entry_meta() -> Dictionary:
	return {
		entries = get_meta("entries", []),
		is_multi = get_meta("is_multi", false),
		paths_arr = get_meta("paths_arr", []),
		global_path = get_meta("global_path", ""),
	}

func _extract_one(name: String, dest: String, m: Dictionary) -> bool:
	if m.is_multi:
		return gseven.extract_entry_multi_volume(m.paths_arr, name, dest, $PassInput.text)
	else:
		return gseven.extract_entry(m.global_path, name, dest, $PassInput.text)

func _extract_one_to_buf(name: String, m: Dictionary) -> PackedByteArray:
	if m.is_multi:
		return gseven.extract_entry_multi_volume_to_buffer(m.paths_arr, name, $PassInput.text)
	else:
		return gseven.extract_entry_to_buffer(m.global_path, name, $PassInput.text)

# ---- Botones ----

func _on_extract_disk_pressed() -> void:
	var entries = get_meta("entries", [])
	if entries.is_empty():
		$Label.text = "Primero lista las entradas"
		return

	var indices = _parse_indices()
	if indices.is_empty():
		$Label.text = "Indices invalidos (ej: 0,2,4 o 'all')"
		return

	var m = _get_entry_meta()
	var dest_base = ProjectSettings.globalize_path(EXTRACT_DIR)
	var text = "Extrayendo a disco (%d entrada(s)):\n\n" % indices.size()

	for idx in indices:
		var e = entries[idx]
		var name = e["name"]
		if e["is_directory"]:
			text += "[SKIP] %s es DIR\n" % name
			continue

		var dest = dest_base + "/" + name
		var ok = _extract_one(name, dest, m)
		if ok:
			var size = _fmt(e["size"])
			text += "[OK] %s  (%s)\n" % [name, size]
		else:
			text += "[ERR] %s\n" % name

	$Label.text = text

func _on_extract_ram_pressed() -> void:
	var entries = get_meta("entries", [])
	if entries.is_empty():
		$Label.text = "Primero lista las entradas"
		return

	var indices = _parse_indices()
	if indices.is_empty():
		$Label.text = "Indices invalidos (ej: 0,2,4 o 'all')"
		return

	var m = _get_entry_meta()
	var dest_base = ProjectSettings.globalize_path(EXTRACT_DIR)
	var text = "Leyendo a buffer (%d entrada(s)):\n\n" % indices.size()

	for idx in indices:
		var e = entries[idx]
		var name = e["name"]
		if e["is_directory"]:
			text += "[SKIP] %s es DIR\n" % name
			continue

		var mem_before = OS.get_static_memory_usage()
		var buf = _extract_one_to_buf(name, m)
		var mem_after = OS.get_static_memory_usage()
		var ram_delta = mem_after - mem_before

		if buf.size() == 0:
			text += "[ERR] %s\n" % name
			continue

		var buf_fmt = _fmt(buf.size())
		text += "[OK] %s  buffer: %s  RAM: +%s\n" % [name, buf_fmt, _fmt(max(ram_delta, 0))]

		# Tambien extraer a disco para tener el archivo
		var dest = dest_base + "/" + name
		_extract_one(name, dest, m)

	$Label.text = text

func _fmt(bytes: int) -> String:
	if bytes < 1024:
		return "%d B" % bytes
	elif bytes < 1048576:
		return "%.1f KB" % (bytes / 1024.0)
	else:
		return "%.2f MB" % (bytes / 1048576.0)

func _gather_volumes(first: String) -> Array[String]:
	var out: Array[String] = []
	var base = first.trim_suffix(".001")
	var i = 1
	while true:
		var vol = base + ".%03d" % i
		if not FileAccess.file_exists(vol):
			break
		out.append(vol)
		i += 1
	return out

func _on_file_selected(path: String) -> void:
	$PathInput.text = path

func _on_browse_pressed() -> void:
	$FileDialog.popup_centered()
