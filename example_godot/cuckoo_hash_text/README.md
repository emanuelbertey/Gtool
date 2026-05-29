# Algoritmo Cuckoo Hash Text

## Descripción
Toma un texto, lo divide en chunks de 3 bytes, agrega un sufijo aleatorio a cada chunk, lo hashea con el CuckooFilter y verifica que todos los chunks estén presentes.

## Partes del algoritmo

### 1. Dividir texto en chunks de 3 bytes
```
Entrada: "hola mundo" (10 bytes)
Salida:  ["hol", "a m", "und", "o"]
```
El texto se convierte a `PackedByteArray` y se recorren los bytes en grupos de 3.

### 2. Generar sufijo aleatorio
```
seed -> RandomNumberGenerator -> N bytes aleatorios (long_sufijo)
```
Se usa una semilla (`seed`) para inicializar un `RandomNumberGenerator` de Godot y generar `long_sufijo` bytes aleatorios. La semilla permite reproducir exactamente el mismo sufijo.

### 3. Hashear cada chunk + sufijo
```
Por cada chunk:
  datos = chunk + sufijo
  hash = CuckooFilter.generate_hash(datos)
  CuckooFilter.add(hash)
```
Se concatenan los 3 bytes del chunk con los bytes del sufijo, se genera un hash con `generate_hash()` y se agrega al `CuckooFilter`.

### 4. Verificar presencia
```
Por cada chunk:
  datos = chunk + sufijo
  hash = CuckooFilter.generate_hash(datos)
  CuckooFilter.contains(hash)
```
Se regenera el hash con los mismos datos (chunk + sufijo) y se consulta al filtro si existe. Todos deben devolver `true`.

### 5. Reportar semilla
```
Seed del sufijo: <número>
```
La semilla usada se imprime al final. Con esa semilla se puede reproducir exactamente el mismo sufijo y verificar los datos.

## Variables configurables
- `long_sufijo`: Longitud del sufijo en bytes (1 a 16). Valor por defecto: 4.

## Uso
1. Abrir la escena `cuckoo_hash_text.tscn`
2. Escribir un texto en el campo de texto
3. Presionar "Hash & Verificar"
4. Ver el resultado en pantalla y consola
