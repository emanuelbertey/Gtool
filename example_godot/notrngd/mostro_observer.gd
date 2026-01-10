extends Node

# Ejemplo de Observer (Solo escucha)
# Basado en: 8e81f2c69990c3fa473752d8c3ead1c085ff12f8f163d4ae7a208994032065a3
var instancia = 5
var nostr_peer: NostrPeer
var timer: Timer
@onready var scroll = $"../observer/ScrollContainer"
# Clave compartida del observer
var shared_key = ""
#var relay = ""
var relay_url: Array[String] = ["wss://relay.mostro.network", "wss://nos.lol", "wss://relay.damus.io"]

func _conect():
	print("\n=== Observer - Mostro Chat Demo ===\n")
	var aux = $"../relayedit".relay_url
	if  aux.size() > 5:
		aux.shuffle()
		relay_url = relay_url#aux.slice(0, instancia)
	else:
		relay_url = relay_url#aux
	prints("se iniciaron : " , instancia , " relays :" , relay_url)
	print("\n=================================\n")
	
	shared_key = $"shared key".text
	# 1. Crear el nodo NostrPeer
	nostr_peer = NostrPeer.new()
	add_child(nostr_peer)
	
	# cambiemos el timepo esta limitado a 10 mensajes 
	nostr_peer.n_seconds = 2345566
	nostr_peer.n_limit =  10
	
	# 2. Inicializar como observer (solo lectura)
	print("Inicializando Observer...")
	var success = nostr_peer.init_observer(shared_key, relay_url)
	
	if success:
		print("✅ Observer inicializado correctamente")
		print("👁️  Modo: Solo lectura (no puede enviar mensajes)")
	else:
		print("❌ Error al inicializar Observer")
		return
	
	# 3. Configurar polling de mensajes
	timer = Timer.new()
	timer.wait_time = 2.0  # Consultar cada 2 segundos
	timer.connect("timeout", Callable(self, "_on_poll_messages"))
	add_child(timer)
	timer.start()
	print("📡 Polling iniciado (cada 2s)...")
	print("👂 Escuchando conversación entre Peer 1 y Peer 2...")

func _on_poll_messages():
	if self.visible == false:
		return
	
	var messages = nostr_peer.poll_messages()

	if messages.size() > 0:
		prints("proseso mensaje")
		$ScrollContainer/mensaje.text += str("\n📨 [OBSERVER] Interceptados " + str(messages.size()) + " mensajes:")
		for msg in messages:
			$ScrollContainer/mensaje.text += str("\n  ----------------------------------------")
			$ScrollContainer/mensaje.text += str("\n  De: " + str(msg["pubkey"]))
			$ScrollContainer/mensaje.text += str("\n  Mensaje: " + str(msg["content"]))
			$ScrollContainer/mensaje.text += str("\n  Timestamp: " +  str(msg["created_at"]))
			$ScrollContainer/mensaje.text += str("\n  ----------------------------------------")
		await get_tree().process_frame
		scroll.scroll_vertical = scroll.get_v_scroll_bar().max_value

	#var messages = nostr_peer.poll_messages()
	#
	#if messages.size() > 0:
		#print("\n📨 [PEER 2] Recibidos ", messages.size(), " mensajes:")
		#for msg in messages:
			#print("  ----------------------------------------")
			#print("  De: ", msg["pubkey"])
			#print("  Mensaje: ", msg["content"])
			#print("  Timestamp: ", msg["created_at"])
			#print("  ----------------------------------------")


# Nota: El observer NO puede enviar mensajes
# Si intentas llamar send_message(), fallará porque no tiene sender_keys


func _on_observer_pressed() -> void:
	_conect()
	pass # Replace with function body.
