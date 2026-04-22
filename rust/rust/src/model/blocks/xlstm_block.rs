// xLSTM Block
// Matches Python: blocks/xlstm_block.py
//
// An xLSTM block is either an sLSTM or mLSTM block.
// Structure:
//   x_res = x
//   x = LayerNorm(x)
//   x = xlstm_layer(x)   (either mLSTM or sLSTM)
//   x = x_res + x        (residual)
//   if ffn:
//     x_res = x
//     x = LayerNorm(x)
//     x = ffn(x)
//     x = x_res + x

use burn::prelude::*;
use burn::module::Module;
use burn::config::Config;

use crate::components::ln::{LayerNorm, LayerNormConfig};
use crate::components::feedforward::{GatedFeedForward, GatedFeedForwardConfig};
use super::mlstm::layer::{MLSTMLayer, MLSTMLayerConfig, MLSTMLayerState};
use super::slstm::layer::{SLSTMLayer, SLSTMLayerConfig, SLSTMLayerState};

/// The type of xLSTM layer in this block.
#[derive(Module, Debug)]
pub enum XLSTMLayerType<B: Backend> {
    MLSTM(MLSTMLayer<B>),
    SLSTM(SLSTMLayer<B>),
}

#[derive(Module, Debug)]
pub struct XLSTMBlock<B: Backend> {
    pub xlstm_norm: LayerNorm<B>,
    pub xlstm: XLSTMLayerType<B>,
    pub ffn_norm: Option<LayerNorm<B>>,
    pub ffn: Option<GatedFeedForward<B>>,
}

#[derive(Config, Debug)]
pub struct XLSTMBlockMlstmConfig {
    pub mlstm: MLSTMLayerConfig,
}

#[derive(Config, Debug)]
pub struct XLSTMBlockSlstmConfig {
    pub slstm: SLSTMLayerConfig,
    pub feedforward: Option<GatedFeedForwardConfig>,
}

impl XLSTMBlockMlstmConfig {
    pub fn init<B: Backend>(&self, device: &B::Device) -> XLSTMBlock<B> {
        let embedding_dim = self.mlstm.embedding_dim;
        let xlstm_norm = LayerNormConfig {
            ndim: embedding_dim,
            weight: true,
            bias: false,
            eps: 1e-5,
            residual_weight: true,
        }
        .init(device);

        let xlstm = XLSTMLayerType::MLSTM(self.mlstm.init(device));

        XLSTMBlock {
            xlstm_norm,
            xlstm,
            ffn_norm: None,
            ffn: None,
        }
    }
}

impl XLSTMBlockSlstmConfig {
    pub fn init<B: Backend>(&self, device: &B::Device) -> XLSTMBlock<B> {
        let embedding_dim = self.slstm.embedding_dim;
        let xlstm_norm = LayerNormConfig {
            ndim: embedding_dim,
            weight: true,
            bias: false,
            eps: 1e-5,
            residual_weight: true,
        }
        .init(device);

        let xlstm = XLSTMLayerType::SLSTM(self.slstm.init(device));

        let (ffn_norm, ffn) = if let Some(ff_config) = &self.feedforward {
            let norm = LayerNormConfig {
                ndim: embedding_dim,
                weight: true,
                bias: false,
                eps: 1e-5,
                residual_weight: true,
            }
            .init(device);
            let ff = ff_config.init(device);
            (Some(norm), Some(ff))
        } else {
            (None, None)
        };

        XLSTMBlock {
            xlstm_norm,
            xlstm,
            ffn_norm,
            ffn,
        }
    }
}

pub enum XLSTMBlockState<B: Backend> {
    MLSTM(MLSTMLayerState<B>),
    SLSTM(SLSTMLayerState<B>),
}

impl<B: Backend> XLSTMBlock<B> {
    pub fn empty_state(&self, batch_size: usize, device: &B::Device) -> XLSTMBlockState<B> {
        match &self.xlstm {
            XLSTMLayerType::MLSTM(l) => XLSTMBlockState::MLSTM(l.empty_state(batch_size, device)),
            XLSTMLayerType::SLSTM(l) => XLSTMBlockState::SLSTM(l.empty_state(batch_size, device)),
        }
    }

    pub fn step(
        &self,
        x: Tensor<B, 2>,
        state: XLSTMBlockState<B>,
    ) -> (Tensor<B, 2>, XLSTMBlockState<B>) {
        let x_res = x.clone();
        let x_normed = self.xlstm_norm.forward(x.unsqueeze_dim::<3>(1)).reshape(x_res.dims());

        let (x_xlstm, new_xlstm_state) = match (&self.xlstm, state) {
            (XLSTMLayerType::MLSTM(l), XLSTMBlockState::MLSTM(s)) => {
                let (y, ns) = l.step(x_normed, s);
                (y, XLSTMBlockState::MLSTM(ns))
            }
            (XLSTMLayerType::SLSTM(l), XLSTMBlockState::SLSTM(s)) => {
                let (y, ns) = l.step(x_normed, s);
                (y, XLSTMBlockState::SLSTM(ns))
            }
            _ => panic!("State/Layer mismatch in XLSTMBlock::step"),
        };

        let x = x_res + x_xlstm;
        let mut x_out = x.clone();

        // Optional FFN (FFN is stateless in this context, just a feedforward)
        if let (Some(ffn_norm), Some(ffn)) = (&self.ffn_norm, &self.ffn) {
            let x_ffn = ffn.forward(ffn_norm.forward(x.clone().unsqueeze_dim::<3>(1))).reshape(x.dims());
            x_out = x + x_ffn;
        }

        (x_out, new_xlstm_state)
    }

    /// Forward: (B, S, D) → (B, S, D)
    pub fn forward(&self, x: Tensor<B, 3>) -> Tensor<B, 3> {
        // Pre-norm + residual for xlstm layer
        let x_normed = self.xlstm_norm.forward(x.clone());
        let x_xlstm = match &self.xlstm {
            XLSTMLayerType::MLSTM(layer) => layer.forward(x_normed),
            XLSTMLayerType::SLSTM(layer) => layer.forward(x_normed),
        };
        let mut x = x + x_xlstm;

        // Pre-norm + residual for optional FFN
        if let (Some(ffn_norm), Some(ffn)) = (&self.ffn_norm, &self.ffn) {
            let x_ffn = ffn.forward(ffn_norm.forward(x.clone()));
            x = x + x_ffn;
        }

        x
    }
}
