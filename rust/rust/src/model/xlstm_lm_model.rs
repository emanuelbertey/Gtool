// xLSTM Language Model
// Matches Python: xlstm_lm_model.py
//
// Structure:
//   token_embedding → dropout → block_stack → lm_head
//
// The LM head projects from embedding_dim to vocab_size.

use burn::prelude::*;
use burn::module::Module;
use burn::config::Config;
use burn::nn;

use crate::xlstm_block_stack::{XLSTMBlockStack, XLSTMBlockStackConfig, XLSTMBlockStackState};

#[derive(Config, Debug)]
pub struct XLSTMLMModelConfig {
    pub vocab_size: usize,
    #[config(default = false)]
    pub add_embedding_dropout: bool,
    // Block stack config fields
    pub block_stack: XLSTMBlockStackConfig,
}

#[derive(Module, Debug)]
pub struct XLSTMLMModel<B: Backend> {
    pub token_embedding: nn::Embedding<B>,
    pub emb_dropout: Option<nn::Dropout>,
    pub xlstm_block_stack: XLSTMBlockStack<B>,
    pub lm_head: nn::Linear<B>,
    pub embedding_dim: usize,
    pub vocab_size: usize,
}

impl XLSTMLMModelConfig {
    pub fn init<B: Backend>(&self, device: &B::Device) -> XLSTMLMModel<B> {
        let embedding_dim = self.block_stack.embedding_dim;

        let token_embedding = nn::EmbeddingConfig::new(self.vocab_size, embedding_dim).init(device);

        let emb_dropout = if self.add_embedding_dropout {
            Some(nn::DropoutConfig::new(self.block_stack.dropout).init())
        } else {
            None
        };

        let xlstm_block_stack = self.block_stack.init(device);

        let lm_head = nn::LinearConfig::new(embedding_dim, self.vocab_size)
            .with_bias(false)
            .init(device);

        XLSTMLMModel {
            token_embedding,
            emb_dropout,
            xlstm_block_stack,
            lm_head,
            embedding_dim,
            vocab_size: self.vocab_size,
        }
    }
}

impl<B: Backend> XLSTMLMModel<B> {
    pub fn empty_state(&self, batch_size: usize, device: &B::Device) -> XLSTMBlockStackState<B> {
        self.xlstm_block_stack.empty_state(batch_size, device)
    }

    pub fn step(
        &self,
        idx: Tensor<B, 1, Int>,
        state: XLSTMBlockStackState<B>,
    ) -> (Tensor<B, 2>, XLSTMBlockStackState<B>) {
        let [b] = idx.dims();
        let idx = idx.reshape([b, 1]); // (B, 1)
        let x = self.token_embedding.forward(idx); // (B, 1, D)
        let x = if let Some(dropout) = &self.emb_dropout {
            dropout.forward(x)
        } else {
            x
        };
        let x = x.reshape([b, self.embedding_dim]); // (B, D)
        let (x, next_state) = self.xlstm_block_stack.step(x, state);
        let logits = self.lm_head.forward(x.unsqueeze_dim::<3>(1)).reshape([b, self.vocab_size]);
        (logits, next_state)
    }

    /// Forward: token indices (B, S) → logits (B, S, vocab_size)
    pub fn forward(&self, idx: Tensor<B, 2, Int>) -> Tensor<B, 3> {
        let x = self.token_embedding.forward(idx); // (B, S, D)
        let x = if let Some(dropout) = &self.emb_dropout {
            dropout.forward(x)
        } else {
            x
        };
        let x = self.xlstm_block_stack.forward(x);     // (B, S, D)
        self.lm_head.forward(x)                         // (B, S, V)
    }
}
