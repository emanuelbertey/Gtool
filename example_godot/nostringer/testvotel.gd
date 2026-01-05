extends Control

func _ready():
	test_votacion_realista()

func test_votacion_realista():
	var n = Nostringer.new()
	print("--- TEST DE VOTACIÓN REALISTA (Simulación de Red) ---")

	var llaves_privadas = []
	var anillo = []
	for i in range(5):
		var kp = n.generate_keypair("xonly")
		llaves_privadas.append(kp["private_key"])
		anillo.append(kp["public_key"])
	
	var urna = []
	print("\n--- Fase de Recepción ---")
	urna.append(enviar_voto_a_red(n, "Propuesta A", llaves_privadas[1], anillo))
	urna.append(enviar_voto_a_red(n, "Propuesta B", llaves_privadas[3], anillo))
	print("[!] Recibiendo paquete sospechoso...")
	urna.append(enviar_voto_a_red(n, "Propuesta C (Trampa)", llaves_privadas[1], anillo))

	procesar_auditoria(n, urna, anillo)

func enviar_voto_a_red(instancia: Nostringer, msg: String, nsec: String, ring: Array) -> Dictionary:
	# CORREGIDO A 5 ARGUMENTOS (añadiendo "") para que no de error el plugin cargado
	var res = instancia.sign(msg.to_utf8_buffer(), nsec, ring, "blsag")
	if not res.has("signature"): 
		return {}
	
	return {
		"sig": res["signature"],
		"msg": msg
	}

func procesar_auditoria(instancia: Nostringer, datos_urna: Array, ring: Array):
	print("\n--- Iniciando Auditoría y Conteo ---")
	var mapa_imagenes = {}
	for i in range(datos_urna.size()):
		var voto = datos_urna[i]
		var verify_res = instancia.verify(voto["sig"], voto["msg"].to_utf8_buffer(), ring)
		if not verify_res.get("valid", false):
			print("Voto %d INVÁLIDO." % i)
			continue
			
		var ki = verify_res.get("key_image", "")
		if mapa_imagenes.has(ki):
			print("\n[!] FRAUDE detectado en Voto %d." % i)
		else:
			mapa_imagenes[ki] = 1
			print("Voto %d validado." % i)
	print("\n--- Auditoría completada. ---")
