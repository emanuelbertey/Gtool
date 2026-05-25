extends Control
var strsim = StringSimilarity.new()
# Mapeo local para asegurar compatibilidad total
const ALGO = {
	"HAMMING": 0,
	"LEVENSHTEIN": 1,
	"NORMALIZED_LEVENSHTEIN": 2,
	"OSA_DISTANCE": 3,
	"DAMERAU_LEVENSHTEIN": 4,
	"NORMALIZED_DAMERAU_LEVENSHTEIN": 5,
	"JARO": 6,
	"JARO_WINKLER": 7,
	"SORENSEN_DICE": 8
}

# Array con texto/escenas para autocompletar
var data_array = [
	"res://example_godot/main.tscn",
	"res://example_godot/menu.tscn",
	"res://example_godot/control.tscn",
	"res://example_godot/exit.tscn",
	"res://example_godot/player.gd",
	"res://example_godot/peerinfo.tscn",
	"res://example_godot/strsim_test/example_strsim.tscn",
	"res://example_godot/filecontrol.tscn",
	"res://addons/menu_principal.tscn",
	"res://levels/level_01.tscn",
	"res://levels/level_02.tscn"
]

var current_results = []

func _ready():
	strsim = StringSimilarity.new()
	$ItemList.hide()

func _test():
	strsim = StringSimilarity.new()
	strsim.algorithm = ALGO.LEVENSHTEIN
	var d = strsim.compare("kitten", "sitting")
	print("Distancia Levenshtein (kitten, sitting): ", d)
	
	strsim.algorithm = ALGO.JARO_WINKLER
	var s = strsim.compare("cheeseburger", "cheese fries")
	print("Similitud Jaro-Winkler: ", s)
	
	var s1 = "rust"
	var s2 = "rustacean"
	var output = "Comparando '%s' vs '%s':\n\n" % [s1, s2]
	
	for name in ALGO.keys():
		var val = ALGO[name]
		var res = strsim.get_similarity(s1, s2, val)
		output += "- %s: %f\n" % [name, res]
	print(output)
	if has_node("Label"):
		$Label.text = output

func _on_line_edit_text_changed(new_text: String) -> void:
	_update_item_list(new_text)

func _update_item_list(search_text: String) -> void:
	var list = $ItemList
	list.clear()
	current_results.clear()
	
	if search_text.is_empty():
		list.hide()
		return
		
	var scored_items = []
	
	# Puntuar 
	for item in data_array:
		var item_lower = item.to_lower()
		var search_lower = search_text.to_lower()
		
		# Para que palabras cortas (ej. "player") no sean destruidas por la enorme longitud
		# de la ruta ("res://.../player.gd"), aplicamos algoritmos tanto a la ruta como al nombre de archivo.
		var file_name = item.get_file().to_lower()
		
		# Calculamos los 3 algoritmos que devuelven un puntaje normalizado (0.0 a 1.0) para la ruta completa
		var jw_full = strsim.get_similarity(search_lower, item_lower, ALGO.JARO_WINKLER)
		var nl_full = strsim.get_similarity(search_lower, item_lower, ALGO.NORMALIZED_LEVENSHTEIN)
		var sd_full = strsim.get_similarity(search_lower, item_lower, ALGO.SORENSEN_DICE)
		
		# Y los calculamos también solo para el nombre de archivo (por si buscaron una palabra corta como "player")
		var jw_file = strsim.get_similarity(search_lower, file_name, ALGO.JARO_WINKLER)
		var nl_file = strsim.get_similarity(search_lower, file_name, ALGO.NORMALIZED_LEVENSHTEIN)
		var sd_file = strsim.get_similarity(search_lower, file_name, ALGO.SORENSEN_DICE)
		
		# Nos quedamos con el puntaje más alto posible entre todas estas métricas "entre palabras"
		var final_score = max(jw_full, max(nl_full, max(sd_full, max(jw_file, max(nl_file, sd_file)))))
			
		scored_items.append({"text": item, "score": final_score})
		
	# Ordenar por el score (de mayor a menor) y, en caso de empate, alfabéticamente por Godot
	scored_items.sort_custom(func(a, b):
		if abs(a.score - b.score) > 0.001:
			return a.score > b.score
		return a.text < b.text
	)
	
	var threshold = 0.50 # Umbral mínimo de similitud para mostrar
	
	# Agregar al ItemList
	for item_data in scored_items:
		if item_data.score >= threshold:
			current_results.append(item_data.text)
			list.add_item("%s (%.2f)" % [item_data.text, item_data.score])

	if current_results.size() > 0:
		list.show()
	else:
		list.hide()

func _on_item_list_item_selected(index: int) -> void:
	if index >= 0 and index < current_results.size():
		var selected_scene = current_results[index]
		if has_node("Label"):
			$Label.text = "Seleccionaste para usar/cargar:\n" + selected_scene
		
		# Llenar el LineEdit con el resultado y ocultar la lista
		$LineEdit.text = selected_scene
		$LineEdit.caret_column = selected_scene.length()
		$ItemList.hide()

func _on_test_pressed() -> void:
	_test()

func _on_exit_pressed() -> void:
	self.queue_free()
