extends Control

var gchat = Gchat.new()
var tools_enabled := false
var streaming := false
var current_assistant_line := ""

@onready var path_input = $VBoxContainer/ConfigSection/PathInput
@onready var pass_input = $VBoxContainer/ConfigSection/PassInput
@onready var prov_list = $VBoxContainer/ChatSection/ProvList
@onready var model_input = $VBoxContainer/ChatSection/ModelInput
@onready var key_custom = $VBoxContainer/ChatSection/KeyCustom
@onready var format_opt = $VBoxContainer/ChatSection/FormatOpt
@onready var msg_input = $VBoxContainer/InputSection/MsgInput
@onready var tools_check = $VBoxContainer/ToolsSection/ToolsCheck
@onready var stream_check = $VBoxContainer/ToolsSection/StreamCheck
@onready var thinking_check = $VBoxContainer/ToolsSection/ThinkingCheck
@onready var budget_input = $VBoxContainer/ToolsSection/BudgetInput
@onready var conversation = $VBoxContainer/ScrollContainer/Conversation
@onready var prov_name_input = $VBoxContainer/MgmtSection/ProvName
@onready var key_input = $VBoxContainer/MgmtSection/KeyInput
@onready var model_list = $VBoxContainer/ChatSection/ModelList
@onready var model_mgmt_input = $VBoxContainer/MgmtSection/ModelInputMgmt
@onready var add_model_btn = $VBoxContainer/MgmtSection/AddModelBtn
@onready var rem_model_btn = $VBoxContainer/MgmtSection/RemModelBtn
@onready var timer = $StreamTimer

func _ready():
	path_input.text = "res://config.json"
	pass_input.text = ""
	model_input.text = "mistralai/mistral-small-3.1-24b-instruct"
	format_opt.add_item("chat/completions")
	format_opt.add_item("messages")
	format_opt.selected = 0
	budget_input.text = "2048"
	timer.timeout.connect(_on_stream_timer)
	prov_list.item_selected.connect(_on_prov_selected)
	model_input.text_changed.connect(_on_model_text_changed)
	gchat.connect("stream_chunk", _on_stream_chunk)
	gchat.connect("stream_reasoning_token", _on_stream_reasoning_token)
	gchat.connect("stream_finished", _on_stream_finished)
	gchat.connect("stream_error", _on_stream_error)

func get_selected_provider() -> String:
	var sel = prov_list.get_selected_items()
	if sel.size() == 0:
		return ""
	return prov_list.get_item_text(sel[0])

func refresh_providers():
	prov_list.clear()
	var provs = gchat.list_providers()
	for p in provs:
		var keys = gchat.get_keys(p)
		if keys.size() > 0:
			prov_list.add_item(p)
	if prov_list.item_count > 0:
		prov_list.select(0)
	refresh_models()

func get_active_key() -> String:
	var custom = key_custom.text.strip_edges()
	if custom != "":
		return custom
	var prov = get_selected_provider()
	if prov == "":
		return ""
	var keys = gchat.get_keys(prov)
	if keys.size() > 0:
		return keys[0]
	return ""

func append_chat(role: String, text: String):
	var tag = "Usuario" if role == "user" else ("Tool" if role == "tool" else "Asistente")
	var colored = "[color=#88ccff]%s:[/color] %s\n" % [tag, text]
	conversation.text += colored

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
	refresh_providers()

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

func _on_add_prov_pressed():
	var name = prov_name_input.text.strip_edges().to_lower()
	if name.is_empty():
		return
	if gchat.add_provider(name):
		log_msg("Proveedor '%s' agregado" % name)
		refresh_providers()
	else:
		log_msg("El proveedor '%s' ya existe" % name, true)

func _on_rem_prov_pressed():
	var prov = get_selected_provider()
	if prov == "":
		return
	if gchat.remove_provider(prov):
		log_msg("Proveedor '%s' eliminado" % prov)
		refresh_providers()

func _on_add_key_pressed():
	var prov = get_selected_provider()
	if prov == "":
		return
	var key = key_input.text.strip_edges()
	if key.is_empty():
		return
	if gchat.add_key(prov, key):
		log_msg("Key agregada a '%s'" % prov)
		key_input.text = ""
	else:
		log_msg("Error al agregar key", true)

func _on_rem_key_pressed():
	var prov = get_selected_provider()
	if prov == "":
		return
	var keys = gchat.get_keys(prov)
	if keys.size() == 0:
		return
	if gchat.remove_key(prov, keys.size() - 1):
		log_msg("Ultima key eliminada de '%s'" % prov)

func _on_add_model_pressed():
	var prov = get_selected_provider()
	if prov == "":
		return
	var model = model_mgmt_input.text.strip_edges()
	if model.is_empty():
		return
	if gchat.add_custom_model(prov, model):
		log_msg("Modelo '%s' agregado a '%s'" % [model, prov])
		model_mgmt_input.text = ""
		refresh_models()
	else:
		log_msg("El modelo '%s' ya existe en '%s'" % [model, prov], true)

func _on_rem_model_pressed():
	var prov = get_selected_provider()
	if prov == "":
		return
	var sel = model_list.get_selected_items()
	if sel.size() == 0:
		return
	var idx = sel[0]
	if gchat.remove_custom_model(prov, idx):
		log_msg("Modelo eliminado de '%s'" % prov)
		refresh_models()

func _on_prov_selected(index: int):
	var prov = prov_list.get_item_text(index)
	var model = gchat.get_provider_model(prov)
	if model != "":
		model_input.text = model
	refresh_models()

func _on_model_selected(index: int):
	if index < 0 or index >= model_list.item_count:
		return
	model_input.text = model_list.get_item_text(index)

func refresh_models():
	model_list.clear()
	var prov = get_selected_provider()
	if prov == "":
		return
	var models = gchat.get_custom_models(prov)
	for m in models:
		model_list.add_item(m)
	if model_list.item_count > 0:
		model_list.select(0)

func _on_model_text_changed(new_text: String):
	var prov = get_selected_provider()
	if prov != "" and new_text.strip_edges() != "":
		gchat.set_provider_model(prov, new_text.strip_edges())

func _on_tools_toggled(toggled_on: bool):
	tools_enabled = toggled_on

func _on_chat_pressed():
	if streaming:
		return
	var prov = get_selected_provider()
	if prov == "":
		log_msg("Selecciona un proveedor", true)
		return
	var model = model_input.text.strip_edges()
	var msg = msg_input.text.strip_edges()
	if prov.is_empty() or model.is_empty() or msg.is_empty():
		log_msg("Completa proveedor, modelo y mensaje", true)
		return
	var key = get_active_key()
	if key.is_empty():
		log_msg("No hay API key para '%s'" % prov, true)
		return
	var fmt_idx = format_opt.selected
	var fmt = format_opt.get_item_text(fmt_idx)
	var thinking = thinking_check.button_pressed
	var budget = int(budget_input.text.strip_edges())
	if budget <= 0:
		budget = 2048
	append_chat("user", msg)
	var user_msg = '{"role":"user","content":"%s"}' % msg.replace('"', '\\"')
	gchat.append_context(user_msg)
	var messages = gchat.get_context()
	if stream_check.button_pressed:
		streaming = true
		current_assistant_line = ""
		conversation.text += "[color=#88ff88]Asistente:[/color] "
		gchat.chat_stream(prov, model, messages, key, tools_enabled, fmt, thinking, budget)
		timer.start()
	else:
		var result = gchat.chat(prov, model, messages, key, tools_enabled, fmt, thinking, budget)
		if result.has("error"):
			append_chat("assistant", "[Error]: " + result["error"])
			return
		var content = result.get("content", "")
		var reasoning = result.get("reasoning", "")
		if reasoning != "":
			append_chat("assistant", "[Razonamiento]: " + reasoning)
		append_chat("assistant", content)

func _on_stream_timer():
	if not streaming:
		timer.stop()
		return
	gchat.poll_stream()

func _on_stream_chunk(token: String):
	current_assistant_line += token
	conversation.text += token

func _on_stream_reasoning_token(token: String):
	pass

func _on_stream_finished(content: String, reasoning: String):
	streaming = false
	timer.stop()
	if reasoning != "":
		conversation.text += "\n[color=#888888][Razonamiento]:[/color] " + reasoning + "\n"
	conversation.text += "\n"

func _on_stream_error(error: String):
	streaming = false
	timer.stop()
	conversation.text += "\n[color=#ff6666][Error]:[/color] " + error + "\n"

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

func log_msg(msg: String, err: bool = false):
	var prefix = "[ERROR] " if err else ""
	conversation.text += prefix + msg  + "\n"
	print("[Gchat] ", msg)

func _on_exit_pressed():
	get_tree().change_scene_to_file("res://example_godot/main.tscn")
