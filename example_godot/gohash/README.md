# GoHash: filtro minimo de 16 bits

`GoHash` es una clase Godot/Rust pensada para guardar un solo fingerprint de 16 bits, sin tablas cuckoo, sin buckets y sin metadatos. Sirve para preguntar si otro dato cae en el mismo hash corto.

No reemplaza a un cuckoo filter completo cuando queres insertar muchos elementos. Es el formato minimo: un solo valor `u16`. La consulta no dice "este dato existe seguro"; dice "este dato cae en el mismo fingerprint de 16 bits".

## Formato

```text
GoHash
+-- target_hash: u16   2 bytes
```

No hay:

- capacidad
- buckets
- contador
- lista de hashes
- tabla de 1024 posiciones
- metadata de serializacion

El estado real es solamente:

```rust
target_hash: u16
```

## Flujo del algoritmo

El flujo copia la idea que ya usa `CuckooFilterGodot`:

1. Godot entrega bytes con `PackedByteArray`.
2. Rust calcula un hash de 64 bits con `SipHasher13::new_with_keys(0, 0)`.
3. Ese hash se devuelve como `i64`.
4. `GoHash` recorta ese hash a 16 bits con `hash & 0xffff`.
5. La clase guarda solamente ese `u16`.

En codigo, con seed:

```rust
let mut hasher = SipHasher13::new_with_keys(seed0, seed1);
datos.as_slice().hash(&mut hasher);
let hash64 = hasher.finish() as i64;
let hash16 = (hash64 as u64 & 0xffff) as u16;
```

## Metodos principales

- `generate_hash(datos: PackedByteArray) -> i64`: crea el hash grande igual que cuckoo.
- `generate_hash_seeded(datos: PackedByteArray, seed0: i64, seed1: i64) -> i64`: crea el hash grande usando seed.
- `hash16_from_hash(hash_val: i64) -> i64`: muestra el recorte a 16 bits.
- `set_from_hash(hash_val: i64) -> i64`: guarda el fingerprint de 16 bits.
- `set_from_bytes(datos: PackedByteArray) -> i64`: crea el hash y guarda el fingerprint en un paso.
- `set_from_bytes_seeded(datos: PackedByteArray, seed0: i64, seed1: i64) -> i64`: crea el hash con seed y guarda el fingerprint.
- `contains_hash(hash_val: i64) -> bool`: compara `hash_val & 0xffff` contra el fingerprint guardado.
- `contains_hash16(hash16: i64) -> bool`: compara directo contra un fingerprint ya recortado.
- `contains_bytes(datos: PackedByteArray) -> bool`: hashea bytes y compara.
- `contains_bytes_seeded(datos: PackedByteArray, seed0: i64, seed1: i64) -> bool`: hashea bytes con seed y compara.
- `count_matches(start, end_exclusive) -> i64`: prueba un rango de numeros desde Rust.
- `storage_bits() -> i64`: devuelve `16`.
- `storage_bytes() -> i64`: devuelve `2`.
- `hash_space() -> i64`: devuelve `65,536`.

## Diferencia contra cuckoo/bloom

Un cuckoo filter guarda muchos fingerprints en una tabla y necesita metadatos para capacidad, buckets, ubicaciones, relocaciones y serializacion.

Un bloom filter guarda un bitset y usa varios hashes o posiciones por elemento. Tambien necesita tamano de bitset, cantidad de hashes y la tabla de bits.

`GoHash` no guarda una tabla. Guarda un solo fingerprint:

```text
contains = (hash16(entrada) == target_hash)
```

Por eso filtra muy rapido y ocupa casi nada, pero acepta falsos positivos por diseno.

## Probabilidad esperada

Un fingerprint de 16 bits tiene `65,536` valores posibles. Si probas `16,777,216` numeros:

```text
16,777,216 / 65,536 = 256
```

Eso significa que para un solo hash de 16 bits esperas alrededor de 256 candidatos. Si uno de esos candidatos es el valor real, el resto son falsos positivos aproximados.

Con `SipHasher13` real la distribucion es probabilistica: normalmente va a caer cerca de 256, pero no tiene por que dar exactamente 256 en todos los targets. Por eso el test Godot acepta un rango razonable alrededor de 256.

## Ejemplo Godot

Abrir:

```text
res://example_godot/gohash/gohash_test.tscn
```

El script crea bytes, calcula `hash64`, guarda el `hash16`, y cuenta cuantos numeros de `0` a `16,777,216` caen en ese mismo fingerprint.
