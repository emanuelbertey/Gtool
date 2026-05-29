# GoHash GPU compute

Este ejemplo corre un compute shader Godot (`#[compute]`, GLSL 450) que calcula SipHash-1-3 emulado con pares `uint low/high`, recorta a 16 bits y devuelve solo candidatos que coinciden con `target_hash16`.

Abrir:

```text
res://example_godot/gohash_gpu/gohash_gpu_test.tscn
```

Nota importante: el proyecto actualmente usa `GL Compatibility` en `project.godot`. Los compute shaders de `RenderingDevice` necesitan backend Vulkan, normalmente Forward+ o Mobile. Si el renderer sigue en compatibilidad OpenGL, el script va a avisar que no hay `RenderingDevice` local disponible.

El shader guarda `0xffffffff` cuando no hay match y el valor original cuando el fingerprint coincide.

## Multi-target

El ejemplo tambien corre:

```text
res://example_godot/gohash_gpu/siphash16_prepare_targets.glsl
res://example_godot/gohash_gpu/siphash16_multi_targets.glsl
```

`siphash16_prepare_targets.glsl` recibe los valores objetivo y calcula sus fingerprints `hash16` en GPU. Despues `siphash16_multi_targets.glsl` usa esos fingerprints para ejecutar:

```text
16,777,216 valores x 512 objetivos
```

El eje `x` del compute shader recorre valores y el eje `y` recorre objetivos. Para evitar devolver gigabytes de candidatos, el shader acumula con `atomicAdd` la cantidad de matches por objetivo y marca si encontro el valor original de cada target.

La CPU solo genera los valores aleatorios y arma buffers. El calculo de `hash16` de cada target ocurre en GPU antes del brute force, en el mismo compute list y con una barrera GPU entre ambas fases.

## Dual thread

Para probar dos tareas GPU en paralelo desde hilos no bloqueantes, abrir:

```text
res://example_godot/gohash_gpu/gohash_gpu_dual_thread_test.tscn
```

La escena crea 2 `Thread`. Cada worker crea su propio `RenderingDevice`, procesa 512 targets y devuelve metricas de tiempo, matches, targets encontrados y promedio por comparacion. El hilo principal solo espera resultados con polling en `_process`.
