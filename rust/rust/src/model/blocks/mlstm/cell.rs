// mLSTM Cell
// Matches Python: blocks/mlstm/cell.py
//
// The cell computes the core mLSTM operation:
// - Takes q, k, v projections
// - Computes input/forget gate pre-activations from concatenated [q, k, v]
// - Delegates to parallel or recurrent backend
// - Applies MultiHeadLayerNorm to the output

use burn::prelude::*;
use burn::module::Module;
use burn::config::Config;
use burn::nn;

use crate::components::ln::{MultiHeadLayerNorm, MultiHeadLayerNormConfig};
use crate::components::init::bias_linspace_init;
use super::backends::{parallel_stabilized_simple, recurrent_step_stabilized_simple};

#[derive(Config, Debug)]
pub struct MLSTMCellConfig {
    pub context_length: usize,
    pub embedding_dim: usize,
    pub num_heads: usize,
}

#[derive(Module, Debug)]
pub struct MLSTMCell<B: Backend> {
    pub igate: nn::Linear<B>,
    pub fgate: nn::Linear<B>,
    pub outnorm: MultiHeadLayerNorm<B>,
    pub num_heads: usize,
    pub embedding_dim: usize,
}

impl MLSTMCellConfig {
    pub fn init<B: Backend>(&self, device: &B::Device) -> MLSTMCell<B> {
        let igate = nn::LinearConfig::new(3 * self.embedding_dim, self.num_heads)
            .with_bias(true)
            .init(device);

        let fgate = nn::LinearConfig::new(3 * self.embedding_dim, self.num_heads)
            .with_bias(true)
            .init(device);

        let outnorm = MultiHeadLayerNormConfig {
            ndim: self.embedding_dim,
            weight: true,
            bias: false,
            eps: 1e-5,
            residual_weight: true,
        }
        .init(self.num_heads, device);

        let mut cell = MLSTMCell {
            igate,
            fgate,
            outnorm,
            num_heads: self.num_heads,
            embedding_dim: self.embedding_dim,
        };

        cell.reset_parameters(device);
        cell
    }
}

impl<B: Backend> MLSTMCell<B> {
    pub fn reset_parameters(&mut self, device: &B::Device) {
        // Forget gate bias: linspace(3.0, 6.0) para asegurar que el olvido sea lento al inicio
        let fgate_bias = bias_linspace_init::<B>(3.0, 6.0, self.num_heads, device);
        self.fgate.bias = Some(burn::module::Param::from_tensor(fgate_bias));

        let fgate_w = Tensor::zeros(self.fgate.weight.dims(), device);
        self.fgate.weight = burn::module::Param::from_tensor(fgate_w);

        let igate_w = Tensor::zeros(self.igate.weight.dims(), device);
        self.igate.weight = burn::module::Param::from_tensor(igate_w);
        
        let igate_bias = Tensor::random(
            [self.num_heads],
            burn::tensor::Distribution::Normal(0.0, 0.1),
            device,
        );
        self.igate.bias = Some(burn::module::Param::from_tensor(igate_bias));
    }

    pub fn forward(&self, q: Tensor<B, 3>, k: Tensor<B, 3>, v: Tensor<B, 3>) -> Tensor<B, 3> {
        let [b, s, _h] = q.dims();
        let dh = self.embedding_dim / self.num_heads;

        // Concatenación para las puertas (gate input)
        let if_gate_input = Tensor::cat(vec![q.clone(), k.clone(), v.clone()], 2);

        // Reorganizar a multi-head: (B, NH, S, DH)
        let q = q.reshape([b, s, self.num_heads, dh]).swap_dims(1, 2);
        let k = k.reshape([b, s, self.num_heads, dh]).swap_dims(1, 2);
        let v = v.reshape([b, s, self.num_heads, dh]).swap_dims(1, 2);

        // Pre-activaciones de las puertas: (B, NH, S, 1)
        let igate_preact = self.igate.forward(if_gate_input.clone())
            .swap_dims(1, 2).unsqueeze_dim(3);

        let fgate_preact = self.fgate.forward(if_gate_input)
            .swap_dims(1, 2).unsqueeze_dim(3);

        // Backend Paralelo
        let h_state = parallel_stabilized_simple(q, k, v, igate_preact, fgate_preact);

        // Normalización y salida: (B, S, H)
        let h_state_norm = self.outnorm.forward(h_state);
        h_state_norm.swap_dims(1, 2).reshape([b, s, self.embedding_dim])
    }

    pub fn step(
        &self,
        q: Tensor<B, 3>,
        k: Tensor<B, 3>,
        v: Tensor<B, 3>,
        mlstm_state: Option<(Tensor<B, 4>, Tensor<B, 4>, Tensor<B, 4>)>,
    ) -> (Tensor<B, 3>, (Tensor<B, 4>, Tensor<B, 4>, Tensor<B, 4>)) {
        let [b, s, _h] = q.dims(); // s suele ser 1 aquí
        let dh = self.embedding_dim / self.num_heads;
        let device = q.device();

        let if_gate_input = Tensor::cat(vec![q.clone(), k.clone(), v.clone()], 2);

        let q_head = q.reshape([b, s, self.num_heads, dh]).swap_dims(1, 2);
        let k_head = k.reshape([b, s, self.num_heads, dh]).swap_dims(1, 2);
        let v_head = v.reshape([b, s, self.num_heads, dh]).swap_dims(1, 2);

        let igate_preact = self.igate.forward(if_gate_input.clone())
            .swap_dims(1, 2).unsqueeze_dim(3);
        let fgate_preact = self.fgate.forward(if_gate_input)
            .swap_dims(1, 2).unsqueeze_dim(3);

        let (c_state, n_state, m_state) = match mlstm_state {
            Some(state) => state,
            None => {
                let c = Tensor::zeros([b, self.num_heads, dh, dh], &device);
                let n = Tensor::zeros([b, self.num_heads, dh, 1], &device);
                // Sincronizado con Python: 0.0 corregido (antes -30.0)
                let m = Tensor::zeros([b, self.num_heads, 1, 1], &device);
                (c, n, m)
            }
        };

        // Backend Recurrente
        let (h_state, new_state) = recurrent_step_stabilized_simple(
            c_state, n_state, m_state, q_head, k_head, v_head, igate_preact, fgate_preact,
        );

        let h_state_norm = self.outnorm.forward(h_state);
        let output = h_state_norm.swap_dims(1, 2).reshape([b, s, self.embedding_dim]);

        (output, new_state)
    }
}