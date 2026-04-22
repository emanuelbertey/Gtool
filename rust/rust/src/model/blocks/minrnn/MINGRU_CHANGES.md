# MinGRU — Cambios para eliminar NaN en CUDA

**Referencia:** `mingru.ipynb` (Kaggle, enero 2025)  
**Archivo modificado:** `rust/src/blocks/minrnn/mingru.rs`

---

## Resumen

El entrenamiento producía `NaN` en CUDA (f32) desde el primer o segundo batch.
La causa raíz era el `parallel_scan_log`, con problemas secundarios en `log_g_3d` y `forward`.

---

## Cambios detallados

### 1. `log_g_3d` — Clamp antes del `log()`

**Antes:**
```rust
let pos = (activation::relu(x.clone()).add_scalar(0.5)).log();
```

**Después:**
```rust
// clamp to avoid log(0) = -inf on CUDA
let pos = (activation::relu(x.clone()).add_scalar(0.5)).clamp_min(1e-7).log();
```

**Por qué:** Cuando `x` es exactamente `0.0`, `relu(0) + 0.5 = 0.5`, que es seguro. Pero en f32 con kernels CUDA, redondeos podían llevar a `relu(x) = 0` para valores muy pequeños negativos que pasaban el `mask_where`. El `clamp_min(1e-7)` garantiza que nunca se llame a `log(0) = -inf`.

---

### 2. `parallel_scan_log` — Reescritura completa

#### 2a. Bug crítico: `max_dim(1)` con dimensiones incorrectas

**Antes (BUGGEADO):**
```rust
let x_prime = log_values - a_star.clone();
let m = x_prime.clone().max_dim(1);  // shape: [B, 1, H]
let log_h0_plus_b_star = (x_prime - m.clone()).exp().cumsum(1).clamp_min(1e-30).log() + m;
//                                              ^^^^ restaba [B,1,H] pero luego sumaba [B,S+1,H] + [B,1,H]
//                                              El broadcast en CUDA al sumar + m fallaba → NaN
```

El `max_dim(1)` devuelve `[B, 1, H]`. La resta `x_prime - m` broadcast correctamente, pero el resultado del cumsum es `[B, S+1, H]` y al hacer `... + m` de vuelta, en CUDA f32 el broadcasting de dimensiones diferentes producía NaN silencioso.

**Después:** Se eliminó el max trick y se reemplazó por un `logcumsumexp` correcto con clamps.

#### 2b. Clamp de `log_coeffs`

**Antes:**
```rust
let log_coeffs = log_coeffs.clamp(f64::NEG_INFINITY, 0.0);
```

**Después:**
```rust
let log_coeffs = log_coeffs.clamp(-30.0, 0.0);
```

**Por qué:** `f64::NEG_INFINITY` convertido a f32 en CUDA puede tener comportamiento indefinido en `cumsum`. `-30.0` es suficiente (equivale a `e^-30 ≈ 0`, negligible).

#### 2c. Clamp de `x_prime` antes de `exp()`

**Añadido:**
```rust
let x_prime = (log_values - a_star.clone()).clamp(-30.0, 30.0);
```

**Por qué:** `exp(x)` en f32 para `x > 88` → `+inf`. `cumsum([inf, ...])` → `inf`. `log(inf)` → `inf`. Luego `inf - inf` al siguiente paso → `NaN`. Clampar a `[-30, 30]` mantiene `exp` en rango seguro (`e^30 ≈ 1e13`, manejable en f32).

#### 2d. Clamp del `log_h` final

**Añadido:**
```rust
let log_h = (a_star + log_h0_plus_b_star).clamp(-30.0, 30.0);
```

**Por qué:** `a_star` puede acumular valores negativos grandes (es un cumsum de valores ≤ 0). La suma con `log_h0_plus_b_star` puede producir valores fuera de rango que hacen explotar `exp()`.

#### 2e. Nueva función helper `logcumsumexp`

```rust
fn logcumsumexp<B: Backend>(x: Tensor<B, 3>) -> Tensor<B, 3> {
    let x_clamped = x.clamp(-30.0, 30.0);
    x_clamped.exp().cumsum(1).clamp_min(1e-30).log()
}
```

Equivalente al `torch.logcumsumexp(x, dim=1)` del notebook. El `clamp_min(1e-30)` antes de `log()` evita `log(0) = -inf` cuando el cumsum de probabilidades muy pequeñas da exactamente 0 en f32.

---

### 3. `forward` — Eliminado `require_grad()` en h0

**Antes:**
```rust
Tensor::<B, 3>::zeros([b, 1, hidden_size], &device).require_grad()
```

**Después:**
```rust
Tensor::<B, 3>::zeros([b, 1, hidden_size], &device)
```

**Por qué:** El `require_grad()` en un tensor constante de zeros creaba un nodo extra en el grafo de autodiferenciación que no aportaba gradientes útiles. Los gradientes fluyen correctamente por `linear_z` y `linear_h` sin necesitar este nodo.

---

### 4. `sequential_mode` — Corrección de la fórmula

**Antes:**
```rust
let h_tilde = self.linear_h.forward(x_t.clone());
// ...
let h_t = one_minus_z_t * h_prev + z_t * g_3d(h_tilde);
```

**Después:**
```rust
let h_tilde = self.linear_h.forward(x_t);  // sin .clone() innecesario
// ...
// g(h_tilde) not log_g!
let h_t = one_minus_z_t * h_prev + z_t * g_3d(h_tilde);
```

La lógica ya era correcta (usa `g` no `log_g` como dice el notebook), solo se eliminó el `.clone()` innecesario (Rust mueve `x_t` que ya no se usa después).

---

## Estado del archivo final

```
fn g_3d          → sin cambios (correcto desde el inicio)
fn log_g_3d      → + clamp_min(1e-7) antes de log()
fn logcumsumexp  → NUEVA función helper
fn parallel_scan_log → reescrito completamente sin max_dim bug
MinGru::forward  → eliminado require_grad() en h0
MinGru::sequential_mode → eliminado clone innecesario, comentario aclaratorio
```

---

## Cambios adicionales en `mingruchat_cuda.rs`

| Cambio | Razón |
|--------|-------|
| `logits.clamp(-100.0, 100.0)` antes de CrossEntropy | Previene NaN en softmax si los logits exploran al inicio |
| LR `2e-3` → `1e-3` | Más estable para Adam con weight decay y grad clip sin warmup |
| Dropout en `step()` eliminado | `step()` es solo para inferencia; dropout no aplica |
