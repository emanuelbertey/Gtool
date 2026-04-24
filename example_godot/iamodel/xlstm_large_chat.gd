extends XLSTMLargeChat

@export var gen_length: int = 60

@onready var output: RichTextLabel = $"../VBoxContainer/Output"
@onready var input: LineEdit = $"../VBoxContainer/Input"
@onready var gen_button: Button = $"../VBoxContainer/HBoxContainer/GenerateBtn"
@onready var train_button: Button = $"../VBoxContainer/HBoxContainer/TrainBtn"

func _ready() -> void:
	DisplayServer.pip_mode_set_auto_enter_on_background(true)

	gen_button.pressed.connect(_on_generate_pressed)
	train_button.pressed.connect(_on_train_pressed)
	
	gen_button.disabled = true
	train_button.disabled = true
	
	# Si la ruta es la por defecto, usamos user:// para que persista entre ejecuciones
	if tokenizer_path == "tokenizer.json":
		tokenizer_path = "user://tokenizer.json"
		
	# Globalizamos las rutas para que Rust (Burn) no tenga problemas en Windows
	model_file = ProjectSettings.globalize_path(model_file)
	tokenizer_path = ProjectSettings.globalize_path(tokenizer_path)
	
	# Delay inicial para UI
	await get_tree().create_timer(0.1).timeout
	
	var training_path = ProjectSettings.globalize_path("res://../texto.txt")
	
	# Si el tokenizador ya existe, no le pasamos el archivo de texto para evitar re-entrenamiento
	var init_train_arg = training_path
	if FileAccess.file_exists(tokenizer_path):
		init_train_arg = ""
		output.append_text("\n[color=green]Usando tokenizador existente.[/color]")
	else:
		output.append_text("\n[color=yellow]Tokenizador no encontrado, se entrenará uno nuevo...[/color]")
	
	if not init_session(init_train_arg):
		output.append_text("\n[color=red]Error crítico: No se pudo inicializar el modelo.[/color]")
		return

	# Habilitamos botones
	gen_button.disabled = false
	train_button.disabled = false
	
	var model_path = ProjectSettings.globalize_path(model_file)
	if not FileAccess.file_exists(model_path):
		output.append_text("\n[color=orange]Aviso: El modelo aún no tiene pesados entrenados. Pulsa 'Entrenar' para comenzar.[/color]")
	else:
		output.append_text("\n[color=green]Modelo cargado y listo.[/color]")

func _on_generate_pressed():
	var prompt = input.text
	if prompt.is_empty(): return
		
	set_buttons_enabled(false)
	output.append_text("\n\n[b]Input:[/b] " + prompt)
	output.append_text("\n[b]IA:[/b] [color=gray](generando...)[/color]")
	
	await get_tree().process_frame
	var response = generate(prompt, gen_length)
	
	output.append_text("\n" + response)
	set_buttons_enabled(true)
	input.clear()

func _on_train_pressed():
	var training_path = ProjectSettings.globalize_path("res://../texto.txt")
	
	if not FileAccess.file_exists(training_path):
		output.append_text("\n[color=red]Error: No se encuentra 'texto.txt' en la raíz para entrenar.[/color]")
		return
		
	set_buttons_enabled(false)
	output.append_text("\n\n[color=cyan][b]Iniciando entrenamiento...[/b][/color]")
	output.append_text("\n[i]Revisa la consola para ver el progreso detallado (Loss).[/i]")
	
	# Esperar un frame para que se dibuje el texto antes de bloquear la CPU con el entrenamiento
	await get_tree().process_frame
	
	# Llamada a Rust
	if train_on_file(training_path):
		var full_model_path = ProjectSettings.globalize_path(model_file)
		save_model(full_model_path)
		output.append_text("\n[color=green]¡Entrenamiento finalizado con éxito y modelo guardado![/color]")
	else:
		output.append_text("\n[color=red]Hubo un error durante el entrenamiento.[/color]")
	
	set_buttons_enabled(true)

func set_buttons_enabled(enabled: bool):
	gen_button.disabled = !enabled
	train_button.disabled = !enabled
