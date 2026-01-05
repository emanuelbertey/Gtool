extends Control





func _on_nostr_pressed() -> void:
	
	var data_exten = load("res://example_godot/notrngd/nostrn_prueba_observador.tscn").instantiate()

	self.add_child(data_exten)

	prints("⭐️ DATOS AL NODO INSTANCIADO ⭐️" )

	pass # Replace with function body.


func _on_nostr_gift_pressed() -> void:
	var data_exten = load("res://example_godot/notrngd/nostrn_gif.tscn").instantiate()

	self.add_child(data_exten)
	
	prints("⭐️ DATOS AL NODO INSTANCIADO ⭐️" )

	pass # Replace with function body.


func _on_button_pressed() -> void:
	var data_exten = load("res://example_godot/examples-pkarr/main.tscn").instantiate()

	self.add_child(data_exten)
	
	prints("⭐️ DATOS AL NODO INSTANCIADO ⭐️" )
	
	pass # Replace with function body.


func _on_key_pressed() -> void:
	var data_exten = load("res://example_godot/examples-pkarr/key.tscn").instantiate()

	self.add_child(data_exten)
	
	prints("⭐️ DATOS AL NODO INSTANCIADO ⭐️" )


func _on_nostronger_pressed() -> void:
	var data_exten = load("res://example_godot/nostringer/signature_txt.tscn").instantiate()

	self.add_child(data_exten)
	
	prints("⭐️ DATOS AL NODO INSTANCIADO ⭐️" )
	pass # Replace with function body.
