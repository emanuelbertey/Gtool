extends Node

func _ready():
	# El número entero que mencionaste (Timestamp Unix)
	# Nota: 17835545 corresponde al 24 de Julio de 1970
	var timestamp: int = 17835545 
	
	print("--- Ejemplo de Conversión de Fecha ---")
	print("Timestamp original (Entero): ", timestamp)
	
	# 1. Obtener una cadena de texto estándar (YYYY-MM-DD HH:MM:SS)
	var fecha_iso = Time.get_datetime_string_from_unix_time(timestamp)
	print("Resultado ISO 8601: ", fecha_iso)
	
	# 2. Obtener un diccionario para personalizar el formato
	var dict = Time.get_datetime_dict_from_unix_time(timestamp)
	
	# Formato personalizado estilo Latino: DD/MM/YYYY HH:MM:SS
	var fecha_formateada = "%02d/%02d/%d %02d:%02d:%02d" % [
		dict.day, 
		dict.month, 
		dict.year, 
		dict.hour, 
		dict.minute,
		dict.second
	]
	
	print("Resultado Formateado: ", fecha_formateada)
	print("---------------------------------------")
