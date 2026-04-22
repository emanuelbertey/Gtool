// Gated FeedForward network
// Matches Python: components/feedforward.py

use burn::prelude::*;
use burn::module::Module;
use burn::config::Config;
use burn::nn;

#[derive(Config, Debug)]
pub struct GatedFeedForwardConfig {
    pub embedding_dim: usize,
    #[config(default = 1.3)]
    pub proj_factor: f64,
    #[config(default = false)]
    pub bias: bool,
    #[config(default = 0.0)]
    pub dropout: f64,
}

impl GatedFeedForwardConfig {
    pub fn proj_up_dim(&self) -> usize {
        let raw = self.proj_factor * self.embedding_dim as f64;
        let multiple = 64usize;
        let mult = (raw / multiple as f64).ceil() as usize;
        mult * multiple
    }
}

#[derive(Module, Debug)]
pub struct GatedFeedForward<B: Backend> {
    pub proj_up: nn::Linear<B>,
    pub proj_down: nn::Linear<B>,
    pub dropout: nn::Dropout,
    pub proj_up_dim: usize,
}

impl GatedFeedForwardConfig {
    pub fn init<B: Backend>(&self, device: &B::Device) -> GatedFeedForward<B> {
        let proj_up_dim = self.proj_up_dim();
        let proj_up = nn::LinearConfig::new(self.embedding_dim, 2 * proj_up_dim)
            .with_bias(self.bias)
            .init(device);
        let proj_down = nn::LinearConfig::new(proj_up_dim, self.embedding_dim)
            .with_bias(self.bias)
            .init(device);
        let dropout = nn::DropoutConfig::new(self.dropout).init();

        GatedFeedForward {
            proj_up,
            proj_down,
            dropout,
            proj_up_dim,
        }
    }
}

impl<B: Backend> GatedFeedForward<B> {
    /// Forward: (B, S, D) → (B, S, D)
    /// Splits up-projection into gate and value, applies GELU gating.
    pub fn forward(&self, x: Tensor<B, 3>) -> Tensor<B, 3> {
        let up = self.proj_up.forward(x); // (B, S, 2*proj_up_dim)
        let chunks = up.chunk(2, 2); // split along last dim
        let gate_preact = chunks[0].clone();
        let up_proj = chunks[1].clone();
        let gated = burn::tensor::activation::gelu(gate_preact) * up_proj;
        self.dropout.forward(self.proj_down.forward(gated))
    }
}
