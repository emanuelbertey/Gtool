extends Node

func _ready():
	var shamir = Shamir.new()
	
	# 1. Datos del secreto (ejemplo: 64 bytes con valor 42)
	var data = PackedByteArray()
	for i in range(64):
		data.append(42)
	
	var count = 5
	var threshold = 4
	
	print("Creando %d participaciones con umbral %d..." % [count, threshold])
	
	# 2. Crear las participaciones (shares)
	var shares = shamir.create_shares(data, count, threshold)
	if shares == null:
		print("Error al crear shares")
		return
	
	# 3. Perder una participación (borramos el índice 3)
	var shares_copy = shares.duplicate()
	shares_copy.remove_at(3)
	print("Participación eliminada. Quedan: ", shares_copy.size())
	
	##verify
	#prints("tamaño del shares :", shares_copy[0].size())
	#
	#
	# 4. Restaurar el secreto (debe funcionar con 4 participaciones)
	var restored = shamir.combine_shares(shares_copy)
	if restored != null:
		if restored == data:
			print("ÉXITO: Secreto restaurado correctamente con 4 participaciones")
	else:
		print("ERROR: No se pudo restaurar con 4 participaciones")
		
	# 5. Perder otra participación (debe fallar con 3 participaciones ya que el umbral es 4)
	shares_copy.remove_at(0)
	print("Otra participación eliminada. Quedan: ", shares_copy.size())
	
	var restored2 = shamir.combine_shares(shares_copy)
	if restored2 == null:
		print("ÉXITO: El secreto se reporta como perdido con 3 participaciones (bajo el umbral)")
