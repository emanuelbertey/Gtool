extends Control
@onready var chek = $CheckButton
var number1
var number2 
var xor_result

func _ready():
	# Pruebas con diferentes longitudes de bits
	var my_seed = "Godot Rocks".hash()
	seed(my_seed)
	var a = randf() + randi()
	seed(my_seed)
	var b = randf() + randi()
	var bytes_8 = PackedByteArray([34])  # 8 bits
	var bytes_16 = PackedByteArray([0x12, 0x34])  # 16 bits (2 bytes: 0x1234)
	var bytes_128 = PackedByteArray([0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC, 0xDE, 0xF0, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88])  # 128 bits (16 bytes)
	var bytes_large = PackedByteArray()
	for i in range(4096):
		bytes_large.append(randi() % 256)  # Generar 4096 bytes aleatorios (mayor a 4k bytes)
	#
	## Obtener las posiciones de los bits
	#var bit_positions_8 = get_bit_positions(bytes_8, 8)
	#var bit_positions_16 = get_bit_positions(bytes_16, 16)
	#var bit_positions_128 = get_bit_positions(bytes_128, 128)
	#var bit_positions_large = get_bit_positions(bytes_large, 4096 * 8)  # 4096 bytes * 8 bits por byte
#
	## Imprimir las posiciones de los bits
	#print("8 bits:")
	#print_bits(bit_positions_8)
	#
	#print("16 bits:")
	#print_bits(bit_positions_16)
	#
	#print("128 bits:")
	#print_bits(bit_positions_128)
	#
	#print("4096 bytes:")
	#print_bits(bit_positions_large)

	## Operaciones adicionales para mostrar los bits de las operaciones
	#number1 = PackedByteArray([0x06, 0xbd, 0x8f, 0xe7, 0xb0, 0x9b, 0xad, 0x03])
	#number2 = PackedByteArray([0x4b, 0x9a, 0x5a, 0x41, 0x29, 0xdf, 0x04, 0x70])
	# Generar dos números de 8 bytes cada uno
	seed(my_seed)
	number1 = generate_random_8byte_number()
	number2 = generate_random_8byte_number()
	print("Número 1: ", format_bytes(number1))
	print("Número 2 (Clave): ", format_bytes(number2))
	seed(my_seed)
	print("Número ale: ", format_bytes(generate_random_8byte_number()))

# Función para obtener posiciones de bits para longitudes variables
func get_bit_positions(packet: PackedByteArray, bit_length: int) -> Array:
	var bit_positions = []
	
	for byte in packet:
		var byte_bits = []
		for bit in range(8):
			var bit_value = (byte >> (7 - bit)) & 1
			byte_bits.append(bit_value)
		bit_positions.append(byte_bits)
	
	return bit_positions

# Función para imprimir las posiciones de bits
func print_bits(bit_positions: Array) -> void:
	for byte_bits in bit_positions:
		var bit_string = ""
		for bit in byte_bits:
			bit_string += str(bit)
		print(bit_string)

# Función para realizar operaciones lógicas bit a bit
func bitwise_operation(num1: PackedByteArray, num2: PackedByteArray, operation: String) -> PackedByteArray:
	var result = PackedByteArray()
	for i in range(num1.size()):
		var byte1 = num1[i]
		var byte2 = num2[i]
		var res = 0
		match operation:
			"xor":
				res = byte1 ^ byte2
			"and":
				res = byte1 & byte2
			"or":
				res = byte1 | byte2
			"nand":
				res = ~(byte1 & byte2) & 0xFF
			"nor":
				res = ~(byte1 | byte2) & 0xFF
			"xnor":
				res = ~(byte1 ^ byte2) & 0xFF
		result.append(res)
	return result

# Función para formatear los bytes como una cadena hexadecimal
func format_bytes(data: PackedByteArray) -> String:
	var str = ""
	for byte in data:
		str += String("%02x" % byte) + " "
	return str.strip_edges()


func _on_xor_pressed() -> void:
	xor_result = bitwise_operation(number1, number2, "xor")
	print("XOR resultado: ", format_bytes(xor_result))
	if chek.button_pressed == true:
		print_bits(get_bit_positions(xor_result, 64))
	pass # Replace with function body.


func _on_xor_2_pressed() -> void:
	if xor_result == null:
		prints("xor no inicviado , esta vacio ")
		return
	var inverted_xor = bitwise_operation(xor_result, number2, "xor")
	print("Invertido XOR con número 2: ", format_bytes(inverted_xor))
	if chek.button_pressed == true:
		print_bits(get_bit_positions(inverted_xor, 64))
	pass # Replace with function body.


func _on_and_pressed() -> void:

	var and_result = bitwise_operation(number1, number2, "and")
	print("AND resultado: ", format_bytes(and_result))
	if chek.button_pressed == true:
		print_bits(get_bit_positions(and_result, 64))
	pass # Replace with function body.


func _on_or_pressed() -> void:
	var or_result = bitwise_operation(number1, number2, "or")
	print("OR resultado: ", format_bytes(or_result))
	if chek.button_pressed == true:
		print_bits(get_bit_positions(or_result, 64))
	pass # Replace with function body.


func _on_nand_pressed() -> void:
	var nand_result = bitwise_operation(number1, number2, "nand")
	print("NAND resultado: ", format_bytes(nand_result))
	if chek.button_pressed == true:
		print_bits(get_bit_positions(nand_result, 64))
	pass # Replace with function body.


func _on_nor_pressed() -> void:
	var nor_result = bitwise_operation(number1, number2, "nor")
	print("NOR resultado: ", format_bytes(nor_result))
	if chek.button_pressed == true:
		print_bits(get_bit_positions(nor_result, 64))
	pass # Replace with function body.


func _on_xnor_pressed() -> void:
	var xnor_result = bitwise_operation(number1, number2, "xnor")
	print("XNOR resultado: ", format_bytes(xnor_result))
	if chek.button_pressed == true:
		print_bits(get_bit_positions(xnor_result, 64))
	pass # Replace with function body.


# Función para generar un número aleatorio de 8 bytes
func generate_random_8byte_number() -> PackedByteArray:
	var number = PackedByteArray()
	for i in range(8):
		number.append(randi() % 256)
	return number
