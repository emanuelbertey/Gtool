extends Node
#
#func _ready():
	#test_group_signatures()
#
#func test_group_signatures():
	#var nostringer = Nostringer.new()
	#print("--- Iniciando Prueba de Firmas de Grupo (Anillo) ---")
#
	## 1. Crear un grupo de 5 personas (un anillo)
	#var private_keys = []
	#var public_keys = []
	#
	#for i in range(5):
		#var kp = nostringer.generate_keypair("xonly")
		#private_keys.append(kp["private_key"])
		#public_keys.append(kp["public_key"])
		#print("Miembro %d añadido al anillo: %s" % [i, kp["public_key"].left(16) + "..."])
#
	#var message = "Propuesta de votación: ¿Aumentar el presupuesto de café?".to_utf8_buffer()
#
	## 2. El Miembro 1 firma la propuesta (SAG - Anónimo)
	#print("\n[Miembro 1] Firmando propuesta con SAG...")
	#var sig1 = nostringer.sign(message, private_keys[1], public_keys, "sag")
	#
	## 3. El Miembro 4 firma la misma propuesta (SAG - Anónimo)
	#print("[Miembro 4] Firmando propuesta con SAG...")
	#var sig4 = nostringer.sign(message, private_keys[4], public_keys, "sag")
#
	## 4. Verificación: Cualquiera puede verificar que ALGUIEN del grupo firmó
	#var v1 = nostringer.verify(sig1, message, public_keys)
	#var v4 = nostringer.verify(sig4, message, public_keys)
#
	#print("\n--- Resultados de Verificación (SAG) ---")
	#print("Firma 1 es válida por el grupo: ", v1)
	#print("Firma 2 es válida por el grupo: ", v4)
	#print("Nota: No hay forma de saber quién firmó cada una (Anonymity).")
#
	## 5. Prueba de Linkability (BLSAG)
	## Si el Miembro 2 firma dos mensajes distintos con BLSAG, podemos saber que es la misma persona
	#print("\n--- Probando Linkability con BLSAG ---")
	#var msg_a = "Voto: SI".to_utf8_buffer()
	#var msg_b = "Voto: NO".to_utf8_buffer()
	#
	#var sig_link1 = nostringer.sign(msg_a, private_keys[2], public_keys, "blsag")
	#var sig_link2 = nostringer.sign(msg_b, private_keys[2], public_keys, "blsag")
	#
	#print("Firma A (Miembro 2): ", sig_link1.left(30) + "...")
	#print("Firma B (Miembro 2): ", sig_link2.left(30) + "...")
	#
	#var v_link1 = nostringer.verify(sig_link1, msg_a, public_keys)
	#var v_link2 = nostringer.verify(sig_link2, msg_b, public_keys)
	#
	#print("Verificación BLSAG A: ", v_link1)
	#print("Verificación BLSAG B: ", v_link2)
	#
	#assert(v_link1, "Firma A inválida")
	#assert(v_link2, "Firma B inválida")
	#
	#print("\n--- Prueba de Grupo Completada ---")
