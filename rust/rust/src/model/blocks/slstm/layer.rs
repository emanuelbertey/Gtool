// sLSTM Layer
// Matches Python: blocks/slstm/layer.py
//
// x → conv1d? → SiLU → headwise projections for i,f,z,o gates
//   → concat → sLSTM cell → group_norm → output

use burn::prelude::*;
use burn::module::Module;
use burn::config::Config;
use burn::nn;

use crate::components::conv::{CausalConv1d, CausalConv1dConfig};
use crate::components::linear_headwise::{LinearHeadwiseExpand, LinearHeadwiseExpandConfig};
use crate::components::ln::{MultiHeadLayerNorm, MultiHeadLayerNormConfig};
use super::cell::{SLSTMCell, SLSTMCellConfig, SLSTMState};

#[derive(Config, Debug)]
pub struct SLSTMLayerConfig {
    pub embedding_dim: usize,
    #[config(default = 4)]
    pub num_heads: usize,
    #[config(default = 4)]
    pub conv1d_kernel_size: usize,
    #[config(default = 0.0)]
    pub dropout: f64,
}

#[derive(Module, Debug)]
pub struct SLSTMLayer<B: Backend> {
    pub conv1d: Option<CausalConv1d<B>>,
    /// Input gate projection
    pub igate: LinearHeadwiseExpand<B>,
    /// Forget gate projection
    pub fgate: LinearHeadwiseExpand<B>,
    /// Cell input gate projection
    pub zgate: LinearHeadwiseExpand<B>,
    /// Output gate projection
    pub ogate: LinearHeadwiseExpand<B>,
    /// sLSTM cell
    pub slstm_cell: SLSTMCell<B>,
    /// Group normalization (per-head)
    pub group_norm: MultiHeadLayerNorm<B>,
    /// Dropout
    pub dropout: nn::Dropout,
    pub embedding_dim: usize,
    pub num_heads: usize,
}

impl SLSTMLayerConfig {
    pub fn init<B: Backend>(&self, device: &B::Device) -> SLSTMLayer<B> {
        let conv1d = if self.conv1d_kernel_size > 0 {
            Some(
                CausalConv1dConfig {
                    feature_dim: self.embedding_dim,
                    kernel_size: self.conv1d_kernel_size,
                    bias: true,
                }
                .init(device),
            )
        } else {
            None
        };

        let make_gate = |device: &B::Device| -> LinearHeadwiseExpand<B> {
            LinearHeadwiseExpandConfig {
                in_features: self.embedding_dim,
                num_heads: self.num_heads,
                expand_factor_up: 1.0,
                bias: false,
            }
            .init(device)
        };

        let igate = make_gate(device);
        let fgate = make_gate(device);
        let zgate = make_gate(device);
        let ogate = make_gate(device);

        let slstm_cell = SLSTMCellConfig {
            hidden_size: self.embedding_dim,
            num_heads: self.num_heads,
        }
        .init(device);

        let group_norm = MultiHeadLayerNormConfig {
            ndim: self.embedding_dim,
            weight: true,
            bias: false,
            eps: 1e-5,
            residual_weight: true,
        }
        .init(self.num_heads, device);

        let dropout = nn::DropoutConfig::new(self.dropout).init();

        SLSTMLayer {
            conv1d,
            igate,
            fgate,
            zgate,
            ogate,
            slstm_cell,
            group_norm,
            dropout,
            embedding_dim: self.embedding_dim,
            num_heads: self.num_heads,
        }
    }
}

/// sLSTM layer state: (cell_state, conv_state)
pub type SLSTMLayerState<B> = (SLSTMState<B>, Option<Tensor<B, 3>>);

impl<B: Backend> SLSTMLayer<B> {
    pub fn empty_state(&self, batch_size: usize, device: &B::Device) -> SLSTMLayerState<B> {
        let cell = (
            Tensor::zeros([batch_size, self.embedding_dim], device),
            Tensor::zeros([batch_size, self.embedding_dim], device),
            Tensor::zeros([batch_size, self.embedding_dim], device),
            Tensor::zeros([batch_size, self.embedding_dim], device),
        );
        let conv = self.conv1d.as_ref().map(|c| c.empty_state(batch_size, device));
        (cell, conv)
    }

    pub fn step(
        &self,
        x: Tensor<B, 2>,
        state: SLSTMLayerState<B>,
    ) -> (Tensor<B, 2>, SLSTMLayerState<B>) {
        let (cell_state, conv_state_opt) = state;
        let [b, _d] = x.dims();
        let dh = self.embedding_dim / self.num_heads;

        // Optional causal conv + SiLU
        let (x_conv, new_conv_state) = if let (Some(conv), Some(st)) = (&self.conv1d, conv_state_opt)
        {
            let (y, ns) = conv.step(x.clone(), st);
            (burn::tensor::activation::silu(y), Some(ns))
        } else {
            (x.clone(), None)
        };

        // Gate projections
        let i = self.igate.forward(x_conv.clone().unsqueeze_dim::<3>(1));
        let f = self.fgate.forward(x_conv.unsqueeze_dim::<3>(1));
        let z = self.zgate.forward(x.clone().unsqueeze_dim::<3>(1));
        let o = self.ogate.forward(x.unsqueeze_dim::<3>(1));

        let gates = Tensor::cat(vec![i, f, z, o], 2).reshape([b, 4 * self.embedding_dim]);

        // sLSTM cell step
        let (y, new_cell_state) = self.slstm_cell.step(gates, cell_state);

        let y = self.dropout.forward(y.unsqueeze_dim::<3>(1)).reshape([b, self.embedding_dim]);

        // Group norm
        let y_heads = y
            .reshape([b, 1, self.num_heads, dh])
            .swap_dims(1, 2); // (B, NH, 1, DH)
        let y_norm = self.group_norm.forward(y_heads);
        let output = y_norm
            .swap_dims(1, 2)
            .reshape([b, self.embedding_dim]);

        (output, (new_cell_state, new_conv_state))
    }

    /// Forward: (B, S, D) → (B, S, D)
    pub fn forward(&self, x: Tensor<B, 3>) -> Tensor<B, 3> {
        self.forward_with_state(x, None).0
    }

    /// Forward with state: returns (output, final_state)
    pub fn forward_with_state(
        &self,
        x: Tensor<B, 3>,
        state: Option<SLSTMLayerState<B>>,
    ) -> (Tensor<B, 3>, SLSTMLayerState<B>) {
        let [b, s, _d] = x.dims();
        let dh = self.embedding_dim / self.num_heads;
        let device = x.device();

        let (cell_state, conv_state) = match state {
            Some(st) => (Some(st.0), st.1),
            None => (None, None),
        };

        // Optional causal conv + SiLU
        let x_conv = if let Some(conv) = &self.conv1d {
            burn::tensor::activation::silu(conv.forward(x.clone()))
        } else {
            x.clone()
        };

        // Gate projections
        let i = self.igate.forward(x_conv.clone());
        let f = self.fgate.forward(x_conv);
        let z = self.zgate.forward(x.clone());
        let o = self.ogate.forward(x);

        let gates = Tensor::cat(vec![i, f, z, o], 2);

        // sLSTM cell
        let (y, new_cell_state) = self.slstm_cell.forward(gates, cell_state);

        // Dropout & Group norm
        let y = self.dropout.forward(y);
       
        // Group norm: reshape to (B, NH, S, DH) for MultiHeadLayerNorm
       
        let y_heads = y.reshape([b, s, self.num_heads, dh]).swap_dims(1, 2);
        // (B, NH, S, DH) → (B, S, NH, DH) → (B, S, D)
        let y_norm = self.group_norm.forward(y_heads);
        let output = y_norm.swap_dims(1, 2).reshape([b, s, self.embedding_dim]);

        (output, (new_cell_state, conv_state))
    }
}
