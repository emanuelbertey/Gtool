extends Control

var hf = HFGodot.new()
var is_initialized = false

@onready var token_input = $VBoxContainer/TokenSection/TokenInput
@onready var repo_input = $VBoxContainer/RepoSection/RepoInput
@onready var repo_type_opt = $VBoxContainer/RepoSection/RepoTypeOpt
@onready var check_private = $VBoxContainer/RepoSection/CheckPrivate

@onready var local_path_input = $VBoxContainer/FileSection/LocalPathInput
@onready var path_in_repo_input = $VBoxContainer/FileSection/PathInRepoInput
@onready var commit_msg_input = $VBoxContainer/FileSection/CommitMsgInput

@onready var download_file_input = $VBoxContainer/DownloadSection/DownloadFileInput
@onready var download_dir_input = $VBoxContainer/DownloadSection/DownloadDirInput

@onready var output_label = $VBoxContainer/OutputPanel/OutputLabel

func _ready():
	output_label.text = "Inicia el cliente con tu token de Hugging Face para comenzar."
	# Rellenar algunas opciones por defecto para probar
	token_input.text = ""
	repo_input.text = "mi-usuario/test-gdextension"
	local_path_input.text = "res://example_godot/example_toolsec.gd"
	path_in_repo_input.text = "example_toolsec.gd"
	commit_msg_input.text = "Subido desde GDExtension en Godot"
	download_file_input.text = "config.json"
	download_dir_input.text = "user://hf_downloads"

func log_message(msg: String, is_error: bool = false):
	if is_error:
		output_label.text = "[ERROR] " + msg
		print("[HFGodot ERROR] ", msg)
	else:
		output_label.text = msg
		print("[HFGodot INFO] ", msg)

func _on_btn_init_pressed():
	var token = token_input.text.strip_edges()
	if token.is_empty():
		log_message("Por favor ingresa un token de Hugging Face.", true)
		return
	
	log_message("Inicializando cliente...")
	var success = hf.init_client(token)
	if success:
		is_initialized = true
		log_message("Cliente inicializado con éxito!")
	else:
		is_initialized = false
		log_message("Fallo al inicializar el cliente.", true)

func _on_btn_create_repo_pressed():
	if not is_initialized:
		log_message("Primero debes inicializar el cliente.", true)
		return
	
	var repo_id = repo_input.text.strip_edges()
	var repo_type = repo_type_opt.text.to_lower()
	var private = check_private.button_pressed
	
	log_message("Creando repositorio '%s' (%s, privado: %s)..." % [repo_id, repo_type, private])
	var success = hf.create_repository(repo_id, repo_type, private)
	if success:
		log_message("Repositorio creado con éxito!")
	else:
		log_message("Error al crear el repositorio.", true)

func _on_btn_delete_repo_pressed():
	if not is_initialized:
		log_message("Primero debes inicializar el cliente.", true)
		return
	
	var repo_id = repo_input.text.strip_edges()
	var repo_type = repo_type_opt.text.to_lower()
	
	log_message("Eliminando repositorio '%s' (%s)..." % [repo_id, repo_type])
	var success = hf.delete_repository(repo_id, repo_type)
	if success:
		log_message("Repositorio eliminado del Hub.")
	else:
		log_message("Error al eliminar el repositorio.", true)

func _on_btn_repo_exists_pressed():
	if not is_initialized:
		log_message("Primero debes inicializar el cliente.", true)
		return
	
	var repo_id = repo_input.text.strip_edges()
	var repo_type = repo_type_opt.text.to_lower()
	
	log_message("Verificando existencia de '%s' (%s)..." % [repo_id, repo_type])
	var exists = hf.repo_exists(repo_id, repo_type)
	if exists:
		log_message("¡El repositorio EXISTE en Hugging Face!")
	else:
		log_message("El repositorio NO existe o no es accesible.", true)

func _on_btn_upload_pressed():
	if not is_initialized:
		log_message("Primero debes inicializar el cliente.", true)
		return
	
	var repo_id = repo_input.text.strip_edges()
	var repo_type = repo_type_opt.text.to_lower()
	var local_path = local_path_input.text.strip_edges()
	var path_in_repo = path_in_repo_input.text.strip_edges()
	var commit_msg = commit_msg_input.text.strip_edges()
	
	# Resolver la ruta real de Godot (ej. res:// a path absoluto)
	var real_local_path = ProjectSettings.globalize_path(local_path)
	
	log_message("Subiendo archivo '%s' a '%s' en el repositorio..." % [local_path, path_in_repo])
	var success = hf.upload_file(repo_id, real_local_path, path_in_repo, commit_msg, repo_type)
	if success:
		log_message("¡Archivo subido con éxito!")
	else:
		log_message("Error al subir el archivo.", true)

func _on_btn_download_pressed():
	if not is_initialized:
		log_message("Primero debes inicializar el cliente.", true)
		return
	
	var repo_id = repo_input.text.strip_edges()
	var repo_type = repo_type_opt.text.to_lower()
	var filename = download_file_input.text.strip_edges()
	var local_dir = download_dir_input.text.strip_edges()
	
	var real_local_dir = ProjectSettings.globalize_path(local_dir)
	
	log_message("Descargando '%s' a '%s'..." % [filename, local_dir])
	var result_path = hf.download_file(repo_id, filename, real_local_dir, repo_type)
	if result_path != "":
		log_message("¡Archivo descargado en:\n" + result_path)
	else:
		log_message("Fallo al descargar el archivo.", true)

func _on_btn_file_exists_pressed():
	if not is_initialized:
		log_message("Primero debes inicializar el cliente.", true)
		return
	
	var repo_id = repo_input.text.strip_edges()
	var repo_type = repo_type_opt.text.to_lower()
	var filename = download_file_input.text.strip_edges()
	
	log_message("Verificando si '%s' existe en el repo..." % filename)
	var exists = hf.file_exists(repo_id, filename, repo_type)
	if exists:
		log_message("¡El archivo '%s' EXISTE en el repositorio!" % filename)
	else:
		log_message("El archivo '%s' NO existe en el repositorio." % filename, true)

func _on_btn_delete_file_pressed():
	if not is_initialized:
		log_message("Primero debes inicializar el cliente.", true)
		return
	
	var repo_id = repo_input.text.strip_edges()
	var repo_type = repo_type_opt.text.to_lower()
	var path_in_repo = path_in_repo_input.text.strip_edges()
	
	log_message("Eliminando archivo '%s' del repositorio..." % path_in_repo)
	var success = hf.delete_file(repo_id, path_in_repo, repo_type)
	if success:
		log_message("¡Archivo '%s' eliminado del repositorio!" % path_in_repo)
	else:
		log_message("Fallo al eliminar el archivo.", true)

func _on_btn_list_files_pressed():
	if not is_initialized:
		log_message("Primero debes inicializar el cliente.", true)
		return
	
	var repo_id = repo_input.text.strip_edges()
	var repo_type = repo_type_opt.text.to_lower()
	
	log_message("Listando archivos del repositorio '%s'..." % repo_id)
	var files = hf.list_repo_files(repo_id, true, repo_type)
	
	if files.size() > 0:
		var file_list_str = "Archivos encontrados:\n"
		for file in files:
			file_list_str += " - %s\n" % file
		log_message(file_list_str)
	else:
		log_message("No se encontraron archivos en este repositorio o el repo no existe.", true)

func _on_btn_search_models_pressed():
	if not is_initialized:
		log_message("Primero debes inicializar el cliente.", true)
		return
	
	var repo_id = repo_input.text.strip_edges()
	# Usar el autor (ej: si repo_id es "meta-llama/llama", tomamos "meta-llama")
	var author = repo_id
	if "/" in repo_id:
		author = repo_id.split("/")[0]
		
	log_message("Buscando modelos del autor/organización '%s'..." % author)
	var models = hf.search_models(author, 10)
	
	if models.size() > 0:
		var models_str = "Modelos de %s:\n" % author
		for model in models:
			models_str += " - %s\n" % model
		log_message(models_str)
	else:
		log_message("No se encontraron modelos para el autor '%s'." % author, true)

func _on_exit_pressed():
	get_tree().change_scene_to_file("res://example_godot/main.tscn")
