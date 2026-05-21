# Diagnóstico: Fallos en Extracción Multi-Volumen y Soluciones

## Contexto
El módulo `advanced_unarc.gd` permite extraer archivos comprimidos (7z, zip, rar) desde Godot usando una extensión Rust (`unarc_godot.rs`). El problema principal ocurre al intentar extraer **una sola entrada** de un archivo **multi-volumen** (ej: `.7z.001`, `.7z.002`) especialmente **con contraseña**.

---

## Fallo 1: `extract_entry_with_format_and_password` no soporta multi-volumen

### Síntoma
```
ExternalLibrary { library: "sevenz-rust2", message: "Io(Error { kind: UnexpectedEof, message: "failed to fill whole buffer" }, "")" }
```

### Causa raíz
En `advanced_unarc.gd:394`, la condición `if detected_volumes.size() > 1 and password == ""` solo manejaba multi-volumen **sin contraseña**. Cuando había contraseña, caía en `extract_entry_with_format_and_password` (línea 402).

Esta función en Rust (`unarc_godot.rs:1153`) abre **un solo archivo** con `std::fs::File::open(path)`. La librería `sevenz-rust2` intenta leer el contenido completo del archive pero solo recibe la primera parte (`.001`), por eso tira `UnexpectedEof: "failed to fill whole buffer"` — el buffer nunca se llena porque faltan las partes siguientes.

### Limitación
`extract_entry_with_format_and_password` **no tiene soporte para multi-volumen** en Rust. No concatena las partes `.001`, `.002`, etc.

### Solución aplicada (mitigación)
En lugar de usar la función rota, usamos `read_entry_bytes_with_password` que **sí soporta multi-volumen internamente** (ya que el preview funcionaba con este método). Leemos los bytes en GDScript y los escribimos a disco manualmente:

```gdscript
if detected_volumes.size() > 1:
    var bytes = unarc.read_entry_bytes_with_password(current_archive_path, entry_name, password)
    if bytes.size() > 0:
        DirAccess.make_dir_recursive_absolute(output_file_path.get_base_dir())
        var file = FileAccess.open(global_output_path, FileAccess.WRITE)
        file.store_buffer(bytes)
        file.close()
        success = true
```

---

## Fallo 2: `extract_all_multi_volume` extrae TODO, no una entrada específica

### Síntoma
La función `extract_all_multi_volume` y `extract_all_multi_volume_with_password` extraen **todas las entradas** del archivo, no la entrada seleccionada por el usuario.

### Causa raíz
En `unarc_godot.rs:813-827`, estas funciones llaman a `extract_archive_entries` que itera sobre **todas** las entradas del archive:

```rust
fn extract_archive_entries<R: Read + Seek>(mut archive: UnifiedArchive<R>, out_dir: &Path) -> bool {
    loop {
        match archive.next_entry() {
            Ok(Some(entry)) => { /* extrae TODAS */ }
            Ok(None) => break,
            Err(e) => { return false; }
        }
    }
    true
}
```

### Limitación
No existe en Rust una función `extract_entry_multi_volume_with_password` que extraiga **una sola entrada** de un multi-volumen.

### Por qué podía parecer un "bucle"
1. **Extrae todo el contenido**: Si el archive tiene muchas entradas o archivos grandes, la función tarda mucho tiempo y la UI de Godot se congela (es una llamada síncrona bloqueante).
2. **Sin contraseña en `read_to`**: `extract_archive_entries` usa `archive.read_to(&entry, &mut out_file)` sin pasar la contraseña. Si alguna entrada está encriptada, `read_to` falla o se bloquea intentando descomprimir datos cifrados sin la clave.
3. **`sevenz-rust2` en multi-volumen**: Si las partes del multi-volumen están incompletas o corruptas, `next_entry()` o `read_to()` pueden tardar indefinidamente intentando leer datos que no existen.

### Solución aplicada (mitigación)
Evitamos completamente `extract_all_multi_volume` para la extracción de una sola entrada. Usamos `read_entry_bytes_with_password` que ya maneja correctamente multi-volumen + contraseña y devuelve solo los bytes de la entrada solicitada.

---

## Fallo 3: `extract_archive_entries` no pasa contraseña al leer entradas

### Síntoma
Archivos multi-volumen encriptados fallan al extraer aunque se proporcione contraseña.

### Causa raíz
En `unarc_godot.rs:1297`:
```rust
if let Err(e) = archive.read_to(&entry, &mut out_file) {
```
Usa `read_to()` sin opciones. Debería usar `read_to_with_options()` con la contraseña, pero la función `extract_archive_entries` no recibe la contraseña como parámetro.

### Limitación
Requeriría modificar la firma de `extract_archive_entries` y propagar la contraseña desde `extract_all_multi_volume_with_password`.

### Solución aplicada (mitigación)
No tocamos Rust. Usamos `read_entry_bytes_with_password` desde GDScript que **ya maneja correctamente la contraseña** internamente.

---

## Resumen de Limitaciones Actuales

| Función | Multi-Volumen | Con Contraseña | Extrae 1 entrada | Estado |
|---|---|---|---|---|
| `extract_entry_with_format_and_password` | ❌ | ✅ | ✅ | Rota para multi-volumen |
| `extract_all_multi_volume` | ✅ | ❌ | ❌ (extrae todo) | Parcialmente funcional |
| `extract_all_multi_volume_with_password` | ✅ | ✅* | ❌ (extrae todo) | `read_to` sin pass |
| `read_entry_bytes_with_password` | ✅ | ✅ | ✅ | **Funcional** |

\* La contraseña se pasa en `ArchiveOptions` pero `extract_archive_entries` no la usa en `read_to()`.

---

## Cómo se Mitigó

La solución consiste en **usar la función que ya funciona** (`read_entry_bytes_with_password`) para multi-volumen, y hacer el volcado a disco desde GDScript:

```
┌─────────────────────────────────────────────────┐
│  Flujo anterior (roto)                          │
│  multi-volumen + pass → extract_entry_with_     │
│    format_and_password → sevenz-rust2 abre      │
│    solo .001 → UnexpectedEof ❌                 │
└─────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────┐
│  Flujo actual (funcional)                       │
│  multi-volumen + pass → read_entry_bytes_with_  │
│    password → concatena partes internamente     │
│    → devuelve bytes → GDScript escribe a disco  │
│    → éxito ✅                                   │
└─────────────────────────────────────────────────┘
```

### Trade-off
- **Ventaja**: Funciona sin tocar Rust, sin recompilar la extensión.
- **Desventaja**: Para archivos muy grandes (>1GB), los bytes pasan por RAM antes de escribirse a disco. Para archivos pesados reales, idealmente se debería implementar `extract_entry_multi_volume_with_password` en Rust que haga streaming directo a disco sin pasar por RAM.

---

## Solución Ideal (Requiere modificar Rust)

Crear `extract_entry_multi_volume_with_password` en `unarc_godot.rs`:

```rust
#[func]
pub fn extract_entry_multi_volume_with_password(
    &self,
    paths: Array<GString>,
    format_extension: String,
    entry_name: String,
    dest_path: String,
    password: String,
) -> bool {
    // 1. Concatenar partes multi-volumen
    // 2. Abrir con ArchiveFormat::open_multi_volume_7z/zip con password
    // 3. Iterar hasta encontrar entry_name
    // 4. Usar read_to_with_options con password para streaming directo a disco
    // 5. Sin pasar por RAM
}
```

Esto permitiría extraer una sola entrada de un multi-volumen con contraseña haciendo **streaming directo a disco**, sin cargar todo en RAM.

## Cambio aplicado en el repositorio

- **Rust:** Añadido el campo `password` como `#[export]` en la estructura `Unarc` (archivo `rust/src/unarc_godot.rs`) y se inicializa como vacío en `init()`. Esto expone la propiedad `password` en Godot para su uso desde GDScript.

