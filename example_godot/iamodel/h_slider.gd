extends HSlider


# Called when the node enters the scene tree for the first time.
func _ready() -> void:
	pass # Replace with function body.


# Called every frame. 'delta' is the elapsed time since the previous frame.
func _process(delta: float) -> void:
	pass


func _on_drag_started() -> void:
	$"../XLSTMLargeChat".gen_length = int(round(self.value))
	#prints(int(round(self.value)))



func _on_drag_ended(value_changed: bool) -> void:
	#$Label.text = str(value_changed)
	pass # Replace with function body.


func _on_value_changed(value: float) -> void:
	$Label.text = str(value)
	$"../XLSTMLargeChat".gen_length = int(round(self.value))
	#prints(int(round(self.value)))
	pass # Replace with function body.
