// LinearHeadwiseExpand
// Matches Python: components/linear_headwise.py
//
// A structured projection that expands inputs per-head.
// weight shape: (num_heads, out_features_per_head, in_features_per_head)
// y[..., h, :] = x[..., h, :] @ weight[h].T + bias

use burn::prelude::*;
use burn::module::{Module, Param};
use burn::config::Config;
use burn::tensor::Distribution;

#[derive(Config, Debug)]
pub struct LinearHeadwiseExpandConfig {
    pub in_features: usize,
    pub num_heads: usize,
    #[config(default = 1.0)]
    pub expand_factor_up: f64,
    #[config(default = false)]
    pub bias: bool,
}

impl LinearHeadwiseExpandConfig {
    pub fn out_features(&self) -> usize {
        (self.expand_factor_up * self.in_features as f64).round() as usize
    }

    pub fn init<B: Backend>(&self, device: &B::Device) -> LinearHeadwiseExpand<B> {
        let out_features = self.out_features();
        let out_per_head = out_features / self.num_heads;
        let in_per_head = self.in_features / self.num_heads;

        // Small init: std = sqrt(2 / (5 * in_per_head))
        let std = (2.0 / (5.0 * in_per_head as f64)).sqrt();
        let weight = Tensor::random(
            [self.num_heads, out_per_head, in_per_head],
            Distribution::Normal(0.0, std),
            device,
        );

        let bias = if self.bias {
            Some(Param::from_tensor(Tensor::zeros([out_features], device)))
        } else {
            None
        };

        LinearHeadwiseExpand {
            weight: Param::from_tensor(weight),
            bias,
            num_heads: self.num_heads,
            in_features: self.in_features,
            out_features,
        }
    }
}

#[derive(Module, Debug)]
pub struct LinearHeadwiseExpand<B: Backend> {
    pub weight: Param<Tensor<B, 3>>,
    pub bias: Option<Param<Tensor<B, 1>>>,
    pub num_heads: usize,
    pub in_features: usize,
    pub out_features: usize,
}

impl<B: Backend> LinearHeadwiseExpand<B> {
    /// Forward: input shape (B, S, in_features) -> output (B, S, out_features).
    pub fn forward(&self, x: Tensor<B, 3>) -> Tensor<B, 3> {
        let [b, s, _f] = x.dims();
        let in_per_head = self.in_features / self.num_heads;
        
        // 1. Preparamos X: (B, S, TotalIn) -> (B, S, H, InH) -> (B, H, S, InH)
        // Esto permite que el matmul sea head-wise
        let x = x.reshape([b, s, self.num_heads, in_per_head]).swap_dims(1, 2);

        // 2. Preparamos W: (H, OutH, InH) -> (H, InH, OutH) -> (1, H, InH, OutH)
        // swap_dims(1, 2) equivale a transponer la matriz de cada cabeza (.T en Python)
        let w = self.weight.val()
            .swap_dims(1, 2)
            .unsqueeze_dim(0);

        // 3. Matmul Headwise: (B, H, S, InH) @ (1, H, InH, OutH) -> (B, H, S, OutH)
        let out = x.matmul(w); 

        // 4. Recombinar: (B, H, S, OutH) -> (B, S, H, OutH) -> (B, S, TotalOut)
        let out = out.swap_dims(1, 2).reshape([b, s, self.out_features]);

        // 5. Bias con broadcasting limpio (1, 1, TotalOut)
        if let Some(bias) = &self.bias {
            out + bias.val().reshape([1, 1, self.out_features])
        } else {
            out
        }
    }
}