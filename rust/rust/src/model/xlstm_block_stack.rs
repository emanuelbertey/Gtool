// xLSTM Block Stack
// Matches Python: xlstm_block_stack.py
//
// A stack of xLSTM blocks, each being either mLSTM or sLSTM.
// Uses a block_map to decide which type goes at each position.
// Followed by an optional post-blocks LayerNorm.

use burn::prelude::*;
use burn::module::Module;
use burn::config::Config;

use crate::components::ln::{LayerNorm, LayerNormConfig};
use crate::components::feedforward::GatedFeedForwardConfig;
use crate::blocks::mlstm::block::MLSTMBlockConfig;
use crate::blocks::mlstm::layer::MLSTMLayerConfig;
use crate::blocks::slstm::block::SLSTMBlockConfig;
use crate::blocks::slstm::layer::SLSTMLayerConfig;
use crate::blocks::xlstm_block::{
    XLSTMBlock, XLSTMBlockMlstmConfig, XLSTMBlockSlstmConfig, XLSTMBlockState,
};

#[derive(Config, Debug)]
pub struct XLSTMBlockStackConfig {
    pub embedding_dim: usize,
    #[config(default = 1)]
    pub num_blocks: usize,
    #[config(default = 256)]
    pub context_length: usize,
    #[config(default = true)]
    pub add_post_blocks_norm: bool,
    #[config(default = false)]
    pub bias: bool,
    #[config(default = 0.0)]
    pub dropout: f64,

    // mLSTM config (used for blocks not in slstm_at)
    pub mlstm_block: Option<MLSTMBlockConfig>,
    // sLSTM config (used for blocks in slstm_at)
    pub slstm_block: Option<SLSTMBlockConfig>,

    /// Which block indices should use sLSTM. All others use mLSTM.
    /// Empty means all mLSTM; all indices means all sLSTM.
    #[config(default = "Vec::new()")]
    pub slstm_at: Vec<usize>,
}

impl XLSTMBlockStackConfig {
    /// Create the block map: 0 = mLSTM, 1 = sLSTM
    fn block_map(&self) -> Vec<usize> {
        let mut map = vec![0usize; self.num_blocks];
        for &idx in &self.slstm_at {
            assert!(idx < self.num_blocks, "slstm_at index {} out of range", idx);
            map[idx] = 1;
        }
        // If no mlstm config, all blocks must be sLSTM
        if self.mlstm_block.is_none() {
            for v in map.iter_mut() {
                *v = 1;
            }
        }
        map
    }
}

#[derive(Module, Debug)]
pub struct XLSTMBlockStack<B: Backend> {
    pub blocks: Vec<XLSTMBlock<B>>,
    pub post_blocks_norm: Option<LayerNorm<B>>,
}

impl XLSTMBlockStackConfig {
    pub fn init<B: Backend>(&self, device: &B::Device) -> XLSTMBlockStack<B> {
        let block_map = self.block_map();
        let mut blocks = Vec::with_capacity(self.num_blocks);

        for (_block_idx, &block_type) in block_map.iter().enumerate() {
            match block_type {
                0 => {
                    // mLSTM block
                    let mlstm_cfg = self.mlstm_block.as_ref()
                        .expect("mlstm_block config required for mLSTM blocks");
                    let layer_config = MLSTMLayerConfig {
                        embedding_dim: self.embedding_dim,
                        num_heads: mlstm_cfg.mlstm.num_heads,
                        conv1d_kernel_size: mlstm_cfg.mlstm.conv1d_kernel_size,
                        qkv_proj_blocksize: mlstm_cfg.mlstm.qkv_proj_blocksize,
                        proj_factor: mlstm_cfg.mlstm.proj_factor,
                        bias: self.bias,
                        dropout: self.dropout,
                        context_length: self.context_length,
                    };
                    let block = XLSTMBlockMlstmConfig { mlstm: layer_config }.init(device);
                    blocks.push(block);
                }
                1 => {
                    // sLSTM block
                    let slstm_cfg = self.slstm_block.as_ref()
                        .expect("slstm_block config required for sLSTM blocks");
                    let layer_config = SLSTMLayerConfig {
                        embedding_dim: self.embedding_dim,
                        num_heads: slstm_cfg.slstm.num_heads,
                        conv1d_kernel_size: slstm_cfg.slstm.conv1d_kernel_size,
                        dropout: self.dropout,
                    };
                    let ff_config = slstm_cfg.feedforward.as_ref().map(|ff| {
                        GatedFeedForwardConfig {
                            embedding_dim: self.embedding_dim,
                            proj_factor: ff.proj_factor,
                            bias: self.bias,
                            dropout: self.dropout,
                        }
                    });
                    let block = XLSTMBlockSlstmConfig {
                        slstm: layer_config,
                        feedforward: ff_config,
                    }
                    .init(device);
                    blocks.push(block);
                }
                _ => panic!("Invalid block type {}", block_type),
            }
        }

        let post_blocks_norm = if self.add_post_blocks_norm {
            Some(
                LayerNormConfig {
                    ndim: self.embedding_dim,
                    weight: true,
                    bias: false,
                    eps: 1e-5,
                    residual_weight: true,
                }
                .init(device),
            )
        } else {
            None
        };

        XLSTMBlockStack {
            blocks,
            post_blocks_norm,
        }
    }
}

pub struct XLSTMBlockStackState<B: Backend> {
    pub block_states: Vec<XLSTMBlockState<B>>,
}

impl<B: Backend> XLSTMBlockStack<B> {
    pub fn empty_state(&self, batch_size: usize, device: &B::Device) -> XLSTMBlockStackState<B> {
        let block_states: Vec<XLSTMBlockState<B>> = self
            .blocks
            .iter()
            .map(|b| b.empty_state(batch_size, device))
            .collect();
        XLSTMBlockStackState { block_states }
    }

    pub fn step(
        &self,
        mut x: Tensor<B, 2>,
        state: XLSTMBlockStackState<B>,
    ) -> (Tensor<B, 2>, XLSTMBlockStackState<B>) {
        let mut new_block_states = Vec::with_capacity(self.blocks.len());

        for (block, b_state) in self.blocks.iter().zip(state.block_states.into_iter()) {
            let (y, ns) = block.step(x, b_state);
            x = y;
            new_block_states.push(ns);
        }

        if let Some(norm) = &self.post_blocks_norm {
            x = norm.forward(x.clone().unsqueeze_dim::<3>(1)).reshape(x.dims());
        }

        (x, XLSTMBlockStackState { block_states: new_block_states })
    }

    /// Forward: (B, S, D) → (B, S, D)
    pub fn forward(&self, mut x: Tensor<B, 3>) -> Tensor<B, 3> {
        for block in &self.blocks {
            x = block.forward(x);
        }
        if let Some(norm) = &self.post_blocks_norm {
            x = norm.forward(x);
        }
        x
    }
}
