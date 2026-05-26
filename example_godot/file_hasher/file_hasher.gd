extends Control

@onready var dir_input = $PathInput
@onready var recursive_check = $RecursiveCheck
@onready var hash_btn = $HashBtn
@onready var output = $Output
@onready var file_dialog = $FileDialog

func _on_browse_pressed() -> void:
	file_dialog.popup_centered()

func _on_file_selected(path: String) -> void:
	dir_input.text = path

func _on_dir_selected(path: String) -> void:
	dir_input.text = path

func _on_hash_pressed() -> void:
	var raw = ProjectSettings.globalize_path(dir_input.text)
	if raw.is_empty():
		output.text = "Error: introduce una ruta"
		return

	if FileAccess.file_exists(raw):
		_hash_single(raw)
		return

	if DirAccess.dir_exists_absolute(raw):
		_hash_dir(raw)
		return

	output.text = "Error: la ruta no existe"

func _hash_single(path: String) -> void:
	output.text = "Calculando hash...\n"
	await get_tree().process_frame

	var hash_str = _file_sha256(path)
	if hash_str:
		output.text = "SHA-256  %s\n%s" % [hash_str, path]
	else:
		output.text = "Error: no se pudo leer el archivo"

func _hash_dir(dir: String) -> void:
	output.text = "Calculando hashes...\n"
	await get_tree().process_frame

	var recursive = recursive_check.button_pressed
	var results = {}
	var errors = []

	var files = _walk_dir(dir, recursive)
	for f in files:
		var hash_str = _file_sha256(f)
		if hash_str:
			results[f] = hash_str
		else:
			errors.append(f)

	var text = "SHA-256 (%d archivos):\n\n" % results.size()
	for path in results:
		text += "%s  %s\n" % [results[path], path]

	if errors.size() > 0:
		text += "\nERRORES (%d archivos):\n" % errors.size()
		for e in errors:
			text += "  (no se pudo leer) %s\n" % e

	output.text = text

func _walk_dir(dir: String, recursive: bool) -> PackedStringArray:
	var out = PackedStringArray()
	var d = DirAccess.open(dir)
	if not d:
		return out
	d.list_dir_begin()
	var f = d.get_next()
	while f != "":
		if f == "." or f == "..":
			f = d.get_next()
			continue
		var full = dir + "/" + f
		if d.current_is_dir():
			if recursive:
				out.append_array(_walk_dir(full, true))
		else:
			out.append(full)
		f = d.get_next()
	d.list_dir_end()
	return out

func _file_sha256(path: String) -> String:
	if not FileAccess.file_exists(path):
		return ""
	return FileAccess.get_sha256(path)
