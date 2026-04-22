// Initialization utilities matching Python: small_init_init_, wang_init_, bias_linspace_init_

use burn::prelude::*;

pub fn bias_linspace_init<B: Backend>(start: f32, end: f32, n_dims: usize, device: &B::Device) -> Tensor<B, 1> {
    if n_dims == 0 { return Tensor::zeros([0], device); }
    if n_dims == 1 { return Tensor::zeros([1], device) + start; }
    let step = (end - start) / (n_dims - 1) as f32;
    Tensor::<B, 1, Int>::arange(0..n_dims as i64, device).float() * step + start
}

/// Standard deviation for "small init" — Transformers without Tears.
/// std = sqrt(2 / (5 * dim))
pub fn small_init_std(dim: usize) -> f64 {
    (2.0 / (5.0 * dim as f64)).sqrt()
}

/// Standard deviation for "wang init".
/// std = 2 / num_blocks / sqrt(dim)
pub fn wang_init_std(dim: usize, num_blocks: usize) -> f64 {
    2.0 / num_blocks as f64 / (dim as f64).sqrt()
}
