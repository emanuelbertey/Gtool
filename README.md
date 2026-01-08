# Gtool GDextension / Extensión GD de Gtool

> ES: Extensión para Godot que añade soporte P2P, DNS descentralizado y criptografía con Nostrn y PKARR.  
> EN: GDextension for Godot adding P2P support, decentralized DNS, and cryptography with Nostrn and PKARR.

---

## Descripción / Description

- **ES:** Gtool es una extensión modular para Godot que integra funciones de red descentralizada y seguridad.  
- **EN:** Gtool is a modular extension for Godot that integrates decentralized networking and security.

- **ES:** Incluye clientes Nostrn (simple y con observadores), generación de claves compartidas y soporte para resolución DNS distribuida.  
- **EN:** Includes Nostrn clients (simple and observer), shared key generation, and support for distributed DNS resolution.

---

## Características principales / Key features

- **ES:** Cliente Torrent para compartir y verificar recursos.  
- **EN:** Torrent client for resource sharing and verification.

- **ES:** PKARR DNS: resolución y publicación de registros descentralizados.  
- **EN:** PKARR DNS: decentralized resolution and record publishing.

- **ES:** Nostrn:  
  - Cliente simple para mensajería directa.  
  - Cliente con observadores para monitoreo y difusión.  
- **EN:** Nostrn:  
  - Simple client for direct messaging.  
  - Observer client for monitoring and broadcasting.

- **ES:** Soporte NIP:  
  - NIP-44: mensajes directos.  
  - NIP-59: mensajes cifrados.  
  - NIP-17: mensajes cifrados en grupos.  
- **EN:** NIP support:  
  - NIP-44: direct messages.  
  - NIP-59: encrypted messages.  
  - NIP-17: group encrypted messages.

- **ES:** Generación de claves compartidas para Nostrn y PKARR.  
- **EN:** Shared key generation for Nostrn and PKARR.

- **ES:** Cifrado en anillo para privacidad de remitente.  
- **EN:** Ring encryption for sender privacy.

- **ES:** Shamir Secret Sharing: divide secretos o hashes en múltiples partes, ideal para distribuir seguridad.  
- **EN:** Shamir Secret Sharing: split secrets or hashes into multiple parts, ideal for distributed security.

- **ES:** Cuckoo Filter: almacenamiento de hashes ultra eficiente, reduce el tamaño y permite verificar pertenencia de mensajes o partes de archivos.  
- **EN:** Cuckoo Filter: ultra-efficient hash storage, reduces size and allows checking membership of messages or file parts.

---

## Ejemplos de uso / Usage Examples

### Cuckoo Filter
```gdscript
var cuckoo = CuckooFilterGodot.new()
cuckoo.init_filter(10000, 16) # Capacidad 10k, huella 16 bits

# Generar y añadir hash / Generate and add hash
var data = "mensaje_secreto".to_utf8_buffer()
var h = cuckoo.generate_hash(data)
cuckoo.add(h)

# Verificar persistencia / Verify persistence
cuckoo.save_to_file("user://filtros.bin")
```

### Ring Signature (Privacidad de remitente / Sender privacy)
```gdscript
var nostringer = Nostringer.new()
var message = "voto_secreto".to_utf8_buffer()

# Crear un grupo de claves públicas (el anillo) / Create a group of public keys (the ring)
var ring = [
    "pubkey_1_hex",
    "pubkey_2_hex",
    "mi_propia_pubkey_hex"
]

# Firmar de forma anónima dentro del anillo / Sign anonymously within the ring
# Variante "blsag" permite detectar doble firma sin revelar quién fue
var result = nostringer.sign(message, "mi_clave_privada_hex", ring, "blsag")
var sig = result["signature"]

# Verificar / Verify
var verification = nostringer.verify(sig, message, ring)
if verification["valid"]:
    print("Firma válida y anónima. Key Image: ", verification["key_image"])
```

### Shamir Secret Sharing
```gdscript
var shamir = Shamir.new()
var secret = "mi_clave_secreta".to_utf8_buffer()

# Crear 5 trozos, se necesitan 3 para recuperar / Create 5 shares, 3 needed to recover
var shares = shamir.create_shares(secret, 5, 3)

# Combinar trozos para recuperar / Combine shares to recover
var recovered = shamir.combine_shares([shares[0], shares[2], shares[4]])
print(recovered.get_string_from_utf8())
```

---

## Compatibilidad / Compatibility

- **ES:** Godot 4.6+ en Windows, Linux, macOS y Android (ARM64-v8a).  
- **EN:** Godot 4.6+ on Windows, Linux, macOS, and Android (ARM64-v8a).

---

## Roadmap / Hoja de ruta

- **ES:** Próximas versiones incluirán ejemplos multijugador y soporte NAT traversal.  
- **EN:** Upcoming versions will include multiplayer examples and NAT traversal support.

---

## Licencia / License

- **ES:** MIT. Ver archivo `LICENSE`.  
- **EN:** MIT. See `LICENSE` file.

---

## Contribuciones / Contributions

- **ES:** Se aceptan issues y pull requests con documentación bilingüe y pruebas reproducibles.  
- **EN:** Issues and pull requests with bilingual documentation and reproducible tests are welcome.
