extends Node

# Nostr Demo Script
# Este script demuestra cómo usar el nodo NostrNode implementado en Rust.

var nostr_node: NostrNode
var timer: Timer
var thread = Thread.new()
var messages = []
var instancia = 5
# Credenciales de prueba (Generadas aleatoriamente para el demo)
var my_nsec = "nsec1a2z2njkffkujcxxel0e336dakdgwnynztxkhku2r5sf2a7uslc7q88j0xr"
var peer_npub = "npub1vxuutunznedrszssxzyw6qxn5anc67xxm934vqsj8cdk7ap3ze3qw444pn"

func go_nostrn():
	#if my_nsec == "" or peer_npub == "":
		#my_nsec = $"TabContainer/CHAT 1/nsec".text
		#peer_npub = $"TabContainer/CHAT 1/npub".text

	if my_nsec == "" or peer_npub == "":
		if $"TabContainer/CHAT 1/nsec".text.text == "" or $"TabContainer/CHAT 1/npub".text == "":
			prints("añade las llaves nsec y/o npub validas",peer_npub ,my_nsec)
			return
		my_nsec = $"TabContainer/CHAT 1/nsec".text.text
		peer_npub = $"TabContainer/CHAT 1/npub".text
	elif $"TabContainer/CHAT 1/nsec".text != "" or $"TabContainer/CHAT 1/npub".text != "":
		my_nsec = $"TabContainer/CHAT 1/nsec".text
		peer_npub = $"TabContainer/CHAT 1/npub".text
	else:
		prints("llaves nsec y/o npub validas",my_nsec ,peer_npub)


	#my_nsec = $"TabContainer/CHAT 1/nsec".text
	#peer_npub = $"TabContainer/CHAT 1/npub".text

	if my_nsec == "" or peer_npub == "":
		prints("1 añade las llaves nsec y/o npub validas",peer_npub ,my_nsec)
		return
	
	$"TabContainer/CHAT 1/nsec".visible = false
	$"TabContainer/CHAT 1/npub".visible = false
	
	print("=== Nostr Demo Iniciado ===")
	$"TabContainer/CHAT 2/ScrollContainer/Label".text = "\n === Nostr Demo Iniciado ==="
	
	thread = Thread.new()
	#thread.start(nostr_mensaje.bind(""))
	print("Call iniciado en el árbol")
	
	# 1. Instanciar el nodo Rust
	nostr_node = NostrNode.new()
	add_child(nostr_node)
	
	nostr_node.n_seconds = 2345566
	nostr_node.n_limit =  10
	
	# 2. Inicializar Cliente
	print("1. Inicializando cliente...")
	# Si pasas "" como nsec, generará una nueva identidad
	var success = nostr_node.init_client(my_nsec, peer_npub)
	
	if success:
		print("   ✅ Cliente inicializado")
		print("   🔑 Mi Public Key: ", nostr_node.get_public_key())
	else:
		print("   ❌ Error al inicializar cliente")
		return

	# 3. Configurar Relays
	print("2. Configurando relays...")
	var dm_relays: Array[String] = ["wss://nos.lol"]
	var read_relays: Array[String] = ["wss://relay.primal.net"]
	
	var aux = $TabContainer/relayedit.relay_url
	if  aux.size() > 5:
		aux.shuffle()
		dm_relays = aux.slice(0, instancia)
		aux.shuffle()
		read_relays = aux.slice(0, instancia)
	else:
		dm_relays = aux
		read_relays = aux
	
	
	if nostr_node.add_relays(dm_relays, read_relays):
		print("   ✅ Relays conectados")
	else:
		print("   ❌ Error al conectar relays")

	# 4. Suscribirse
	print("3. Suscribiéndose...")
	if nostr_node.subscribe():
		print("   ✅ Suscripción activa")
	else:
		print("   ❌ Error al suscribirse")

	# 5. Enviar mensaje de prueba
	print("4. Enviando mensaje de prueba...")
	if nostr_node.send_message("Hola desde Godot! " + str(Time.get_unix_time_from_system())):
		print("   ✅ Mensaje enviado")
	
	# 6. Configurar Timer para polling
	# Usamos un timer para no bloquear el main thread en cada frame
	timer = Timer.new()
	timer.wait_time = 5.0 # Consultar cada 2 segundos
	timer.connect("timeout", Callable(self, "_on_timer_timeout"))
	add_child(timer)
	timer.start()
	print("5. Polling iniciado (cada 2s)...")

func _on_timer_timeout():
	print("Consultando mensajes...")
	
	if thread.is_started():
		thread.wait_to_finish()
		thread = Thread.new()
	else:
		thread = Thread.new()
		thread.start(nostr_mensaje.bind(""))

func _process(delta: float) -> void:
	if messages.size() > 0:
		print("📨 Recibidos ", messages.size(), " mensajes nuevos:")
		for msg in messages:
			#print("   ----------------------------------------")
			#print("   De: ", msg["sender"])
			#print("   Mensaje: ", msg["content"])
			#print("   Timestamp: ", msg["timestamp"])
			#print("   ----------------------------------------")
			$"TabContainer/CHAT 1/ScrollContainer/Label".text += "\n " + str( msg["content"])
	messages = []
	
func _on_send_pressed() -> void:
	var msg = ""
	if $"TabContainer/CHAT 1/msg".text == "":
		msg = "Hola desde Godot! "
	else:
		msg = $"TabContainer/CHAT 1/msg".text
	$"TabContainer/CHAT 1/msg".text = ""
	
	if thread.is_started():
		thread.wait_to_finish()
		
		thread.start(enviar_mrnsaje.bind(msg))
	else:
		
		thread.start(enviar_mrnsaje.bind(msg))
	
	#if nostr_node.send_message(msg + str(Time.get_unix_time_from_system())):
		#prints("   ✅ Mensaje enviado")
		$"TabContainer/CHAT 1/ScrollContainer/Label".text += "\n yo envie  :_ " + msg




func nostr_mensaje(msj):
	messages = nostr_node.poll_messages()
	#if messages.size() > 0:
		#print("📨 Recibidos ", messages.size(), " mensajes nuevos:")
		#for msg in messages:
			#print("   ----------------------------------------")
			#print("   De: ", msg["sender"])
			#print("   Mensaje: ", msg["content"])
			#print("   Timestamp: ", msg["timestamp"])
			#print("   ----------------------------------------")
			#$ScrollContainer/Label.text += "\n  A " + str( msg["content"])

	return


func enviar_mrnsaje(mensaje):
	if nostr_node.send_message(mensaje ):#+ str(Time.get_unix_time_from_system())
		prints("   ✅ Mensaje enviado")
	pass

func _exit_tree() -> void:
	if thread.is_started() and not thread == null:
		thread.wait_to_finish()
	pass

func _on_chat_app_pressed() -> void:
	go_nostrn()
	pass # Replace with function body.


func _on_button_pressed() -> void:
	self.queue_free()
	pass # Replace with function body.
