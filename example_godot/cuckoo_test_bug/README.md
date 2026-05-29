# Bug Report: CuckooFilter con hashes duplicados

## Resumen
El `CuckooFilterGodot` (basado en `atomic_cuckoo_filter`) falla catastróficamente cuando se intenta agregar el mismo hash múltiples veces. No solo rechaza los duplicados, sino que **corrompe el estado interno del filtro**, causando que hashes previamente insertados correctamente ya no sean detectados por `contains()`.

## Test realizado
- Capacidad: 1024
- Fingerprint: 16 bits
- 1024 hashes únicos, cada uno insertado 8 veces
- Total de llamadas a `add()`: 8192

## Resultados

### add()
- **7168 fallos** de 8192 llamadas (87.5%)
- Los primeros 3 intentos por hash funcionan
- A partir del 4º intento, `add()` siempre falla
- Esto indica que el filtro acepta hasta 3 copias del mismo hash, pero luego se satura

### contains() (verificación)
- **827 fallos** de 1024 hashes (80.8%)
- Hashes que fueron insertados correctamente ya no se encuentran
- El filtro **pierde entradas** después de procesar duplicados

## Causa probable
El `atomic_cuckoo_filter` maneja los duplicados como si fueran entradas distintas. Cuando el mismo hash se inserta varias veces:
1. Ocupa múltiples slots en los buckets
2. Al no encontrar espacio para más copias, inicia cadenas de "kicks"
3. Durante los kicks, expulsa otras entradas legítimas
4. Las entradas expulsadas se pierden definitivamente

Esto es un **bug de la librería**: un CuckooFilter debería detectar que el hash ya existe y no intentar insertarlo de nuevo, o al menos no corromper otras entradas al hacerlo.

## Conclusión
**No insertar el mismo hash más de una vez** en el `CuckooFilterGodot`. Siempre verificar con `contains()` antes de `add()`, o mantener un registro externo de los hashes ya insertados.
