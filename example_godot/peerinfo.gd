extends TPeer


#
#func _ready() -> void:
	#prints(info_ips())
	#prints(_key("peer_2") ," el peer existe")
	#prints(_key("peer2") ," el peer existe")
	#filetraker(_bytes("sample.torrent"))

func _on_string_format(data: String) -> void:
	prints(data)
	pass # Replace with function body.




func _bytes(path: String) -> PackedByteArray:
	var file := FileAccess.open(path, FileAccess.READ)
	if file == null:
		push_error("No se pudo abrir el archivo: %s" % path)
		
		return PackedByteArray()
	
	var byte_data := file.get_buffer(file.get_length())
	file.close()
	prints(byte_data.size(), " tamalo leido ")
	
	return byte_data


func _on_ips_actualizadas(data: String) -> void:
	prints("ips actualisadfas torrent ", data )
	pass # Replace with function body.
