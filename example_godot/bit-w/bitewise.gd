extends Control

func _ready():
	var packet = PackedByteArray([0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC, 0xDE, 0xF0])
	process_bitwise_operations(packet)

func process_bitwise_operations(packet: PackedByteArray):
	# Generar dos números de 8 bytes cada uno
	var number1 = generate_random_8byte_number()
	var number2 = generate_random_8byte_number()

	# Imprimir los números generados
	print("Número 1: ", format_bytes(number1))
	print("Número 2: ", format_bytes(number2))
	print("Número ale: ", format_bytes(generate_random_8byte_number()))

	# XOR
	var xor_result = bitwise_operation(number1, number2, "xor")
	print("XOR resultado: ", format_bytes(xor_result))
	var inverted_xor = invert_bytes(xor_result)
	print("Invertido XOR resultado: ", format_bytes(inverted_xor))

	# AND
	var and_result = bitwise_operation(number1, number2, "and")
	print("AND resultado: ", format_bytes(and_result))
	var inverted_and = invert_bytes(and_result)
	print("Invertido AND resultado: ", format_bytes(inverted_and))

	# NXOR
	var nxor_result = bitwise_operation(number1, number2, "nxor")
	print("NXOR resultado: ", format_bytes(nxor_result))
	var inverted_nxor = invert_bytes(nxor_result)
	print("Invertido NXOR resultado: ", format_bytes(inverted_nxor))

	# OR
	var or_result = bitwise_operation(number1, number2, "or")
	print("OR resultado: ", format_bytes(or_result))

	# NAND
	var nand_result = bitwise_operation(number1, number2, "nand")
	print("NAND resultado: ", format_bytes(nand_result))

	# NOR
	var nor_result = bitwise_operation(number1, number2, "nor")
	print("NOR resultado: ", format_bytes(nor_result))

	# XNOR
	var xnor_result = bitwise_operation(number1, number2, "xnor")
	print("XNOR resultado: ", format_bytes(xnor_result))

# Función para generar un número aleatorio de 8 bytes
func generate_random_8byte_number() -> PackedByteArray:
	var number = PackedByteArray()
	for i in range(8):
		number.append(randi() % 256)
	return number

# Función para realizar operaciones lógicas bit a bit
func bitwise_operation(num1: PackedByteArray, num2: PackedByteArray, operation: String) -> PackedByteArray:
	var result = PackedByteArray()
	for i in range(num1.size()):
		var byte1 = num1[i] if i < num1.size() else 0
		var byte2 = num2[i] if i < num2.size() else 0
		var res = 0
		match operation:
			"xor":
				res = byte1 ^ byte2
			"and":
				res = byte1 & byte2
			"nxor":
				res = ~(byte1 ^ byte2)
			"or":
				res = byte1 | byte2
			"nand":
				res = ~(byte1 & byte2)
			"nor":
				res = ~(byte1 | byte2)
			"xnor":
				res = ~(byte1 ^ byte2)
			# Agregar más operaciones si es necesario...
		result.append(res)
	return result

# Función para invertir los bytes
func invert_bytes(data: PackedByteArray) -> PackedByteArray:
	var inverted = PackedByteArray()
	for byte in data:
		inverted.append(~byte & 0xFF)  # Asegurar que solo los 8 bits se mantengan
	return inverted

# Función para formatear los bytes como una cadena hexadecimal
func format_bytes(data: PackedByteArray) -> String:
	var str = ""
	for byte in data:
		var hex_str = byte_to_hex(byte)
		str += hex_str + " "  # Agregar cada byte formateado en hexadecimal a la cadena
	return str.strip_edges()

# Función auxiliar para convertir un byte a una cadena hexadecimal
func byte_to_hex(byte: int) -> String:
	var hex_chars = "0123456789abcdef"
	return hex_chars[(byte >> 4) & 0xF] + hex_chars[byte & 0xF]
