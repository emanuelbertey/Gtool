extends Control

var gchat = Gchat.new()
var tools_enabled := false

@onready var path_input = $VBoxContainer/ConfigSection/PathInput
@onready var pass_input = $VBoxContainer/ConfigSection/PassInput
@onready var provider_opt = $VBoxContainer/ChatSection/ProviderOpt
@onready var model_input = $VBoxContainer/ChatSection/ModelInput
@onready var msg_input = $VBoxContainer/ChatSection/MsgInput
@onready var tools_check = $VBoxContainer/ToolsSection/ToolsCheck
@onready var output = $VBoxContainer/OutputPanel/OutputLabel

func _ready():
	path_input.text = "res://../rust/src/FilosoIA/config.json"
	pass_input.text = ""
	provider_opt.text = "openrouter"
	model_input.text = "openai/gpt-4o"

func log_msg(msg: String, err: bool = false):
	var prefix = "[ERROR] " if err else ""
	output.text = prefix + msg
	print("[Gchat] ", msg)

func _on_load_pressed():
	var path = ProjectSettings.globalize_path(path_input.text.strip_edges())
	var pw = pass_input.text.strip_edges()
	if pw.is_empty():
		log_msg("Ingresa la contrasena del config.json", true)
		return
	log_msg("Cargando config...")
	var dict = gchat.load_config(path, pw)
	if dict.is_empty():
		log_msg("Error al cargar config (contrasena incorrecta?)", true)
		return
	log_msg("Config cargada: %d proveedores" % dict.size())
	var provs = gchat.list_providers()
	for p in provs:
		var keys = gchat.get_keys(p)
		log_msg("  %s: %d keys" % [p, keys.size()])

func _on_save_pressed():
	var path = ProjectSettings.globalize_path(path_input.text.strip_edges())
	var pw = pass_input.text.strip_edges()
	if pw.is_empty():
		log_msg("Ingresa la contrasena", true)
		return
	if gchat.save_config(path, pw):
		log_msg("Config guardada correctamente")
	else:
		log_msg("Error al guardar config", true)

func _on_chat_pressed():
	var prov = provider_opt.text.strip_edges().to_lower()
	var model = model_input.text.strip_edges()
	var msg = msg_input.text.strip_edges()
	if prov.is_empty() or model.is_empty() or msg.is_empty():
		log_msg("Completa proveedor, modelo y mensaje", true)
		return
	var keys = gchat.get_keys(prov)
	if keys.is_empty():
		log_msg("No hay keys para '%s'" % prov, true)
		return
	var key = keys[0]
	var messages = '[{"role":"user","content":"%s"}]' % msg.replace('"', '\\"')
	log_msg("Enviando a %s/%s (tools=%s)..." % [prov, model, tools_enabled])
	var result = gchat.chat(prov, model, messages, key, tools_enabled)
	if result.has("error"):
		log_msg("Error: " + result["error"], true)
		return
	if result.has("content"):
		log_msg("Respuesta: " + result["content"])
	else:
		log_msg("Respuesta (raw): " + result.get("body", ""))

func _on_tools_toggled(toggled_on: bool):
	tools_enabled = toggled_on
	log_msg("Tools " + ("activadas" if toggled_on else "desactivadas"))

func _on_list_providers_pressed():
	var provs = gchat.list_providers()
	if provs.is_empty():
		log_msg("No hay proveedores. Carga un config primero.")
		return
	var txt = "Proveedores:\n"
	for p in provs:
		var keys = gchat.get_keys(p)
		txt += "  %s (%d keys)\n" % [p, keys.size()]
	log_msg(txt)

func _on_exit_pressed():
	get_tree().change_scene_to_file("res://example_godot/main.tscn")
