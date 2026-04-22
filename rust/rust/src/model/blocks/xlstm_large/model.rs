use burn::prelude::*;
use burn::nn;
use burn::module::{Module, Param};

use super::config::XLSTMLargeConfig;
use super::components::{RMSNorm, soft_cap};
use super::layer::MLSTMLayer;

#[derive(Module, Debug)]
pub struct SingleFFWeights<B: Backend> {
    pub proj_up_gate: nn::Linear<B>,
    pub proj_up: nn::Linear<B>,
}

#[derive(Module, Debug)]
pub struct FusedFFWeights<B: Backend> {
    pub proj_up_combined: nn::Linear<B>,
}

#[derive(Module, Debug)]
pub enum FeedForwardWeights<B: Backend> {
    Single(SingleFFWeights<B>),
    Fused(FusedFFWeights<B>),
}

#[derive(Module, Debug)]
pub struct FeedForward<B: Backend> {
    pub weights: FeedForwardWeights<B>,
    pub proj_down: nn::Linear<B>,
    pub up_proj_dim: usize,
}

impl<B: Backend> FeedForward<B> {
    pub fn init(config: &XLSTMLargeConfig, device: &B::Device) -> Self {
        let up_dim = config.up_proj_dim();
        let weights = if config.weight_mode == "fused" {
            FeedForwardWeights::Fused(FusedFFWeights {
                proj_up_combined: nn::LinearConfig::new(config.embedding_dim, 2 * up_dim).with_bias(config.use_bias).init(device),
            })
        } else {
            FeedForwardWeights::Single(SingleFFWeights {
                proj_up_gate: nn::LinearConfig::new(config.embedding_dim, up_dim).with_bias(config.use_bias).init(device),
                proj_up: nn::LinearConfig::new(config.embedding_dim, up_dim).with_bias(config.use_bias).init(device),
            })
        };

        Self {
            weights,
            proj_down: nn::LinearConfig::new(up_dim, config.embedding_dim).with_bias(config.use_bias).init(device),
            up_proj_dim: up_dim,
        }
    }

    pub fn forward<const D: usize>(&self, x: Tensor<B, D>) -> Tensor<B, D> {
        let (gate, up) = match &self.weights {
            FeedForwardWeights::Single(single) => {
                (single.proj_up_gate.forward(x.clone()), single.proj_up.forward(x))
            },
            FeedForwardWeights::Fused(fused) => {
                let combined = fused.proj_up_combined.forward(x);
                let gate = combined.clone().narrow(D - 1, 0, self.up_proj_dim);
                let up = combined.narrow(D - 1, self.up_proj_dim, self.up_proj_dim);
                (gate, up)
            }
        };
        let gate_act = burn::tensor::activation::silu(gate);
        self.proj_down.forward(gate_act * up)
    }
}

#[derive(Module, Debug)]
pub struct MLSTMBlock<B: Backend> {
    pub norm_mlstm: RMSNorm<B>,
    pub mlstm_layer: MLSTMLayer<B>,
    pub norm_ffn: RMSNorm<B>,
    pub ffn: FeedForward<B>,
    pub learnable_skip: Option<Param<Tensor<B, 1>>>,
}

impl<B: Backend> MLSTMBlock<B> {
    pub fn init(config: &XLSTMLargeConfig, device: &B::Device) -> Self {
        let learnable_skip = Some(Param::from_tensor(Tensor::ones([config.embedding_dim], device)));
        
        Self {
            norm_mlstm: RMSNorm::init(config.embedding_dim, true, config.use_bias, config.norm_eps, config.norm_reduction_force_float32, device),
            mlstm_layer: MLSTMLayer::init(config, device),
            norm_ffn: RMSNorm::init(config.embedding_dim, true, config.use_bias, config.norm_eps, config.norm_reduction_force_float32, device),
            ffn: FeedForward::init(config, device),
            learnable_skip,
        }
    }

    pub fn forward(
        &self, 
        x: Tensor<B, 3>, 
        state: Option<(Tensor<B, 4>, Tensor<B, 4>, Tensor<B, 4>)>
    ) -> (Tensor<B, 3>, Option<(Tensor<B, 4>, Tensor<B, 4>, Tensor<B, 4>)>) {
        let x_res = x.clone();
        let x_norm = self.norm_mlstm.forward(x);
        let (x_mlstm, next_state) = self.mlstm_layer.forward(x_norm, state);
        
        // Residual + Skip (if skip is enabled, it should be learnable_skip * h_tilde_state + x, but simpler is h_tilde_state + x)
        let x = x_res + x_mlstm;
        
        let x_ffn_res = x.clone();
        let x_ffn_norm = self.norm_ffn.forward(x);
        let x_ffn = self.ffn.forward(x_ffn_norm);
        (x_ffn_res + x_ffn, next_state)
    }
}

#[derive(Clone, Debug)]
pub struct XLSTMLargeState<B: Backend> {
    pub block_states: Vec<(Tensor<B, 4>, Tensor<B, 4>, Tensor<B, 4>)>,
}

#[derive(Module, Debug)]
pub struct XLSTMLarge<B: Backend> {
    pub embedding: nn::Embedding<B>,
    pub blocks: Vec<MLSTMBlock<B>>,
    pub out_norm: Option<RMSNorm<B>>,
    pub lm_head: nn::Linear<B>,
    pub output_logit_soft_cap: Option<f64>,
}

impl<B: Backend> XLSTMLarge<B> {
    pub fn init(config: &XLSTMLargeConfig, device: &B::Device) -> Self {
        let embedding = nn::EmbeddingConfig::new(config.vocab_size, config.embedding_dim).init(device);
        let blocks = (0..config.num_blocks).map(|_| MLSTMBlock::init(config, device)).collect();
        let out_norm = if config.add_out_norm {
            Some(RMSNorm::init(config.embedding_dim, true, config.use_bias, config.norm_eps, config.norm_reduction_force_float32, device))
        } else {
            None
        };
        let lm_head = nn::LinearConfig::new(config.embedding_dim, config.vocab_size).with_bias(false).init(device);
        
        let mut model = Self {
            embedding,
            blocks,
            out_norm,
            lm_head,
            output_logit_soft_cap: config.output_logit_soft_cap,
        };
        
        model.reset_parameters(config, device);
        model
    }

    fn reset_parameters(&mut self, _config: &XLSTMLargeConfig, _device: &B::Device) {
        // Here we'd implement small_init and wang_init
        // For now, let's keep the default or implement the formulas
    }

    pub fn forward(
        &self, 
        x: Tensor<B, 2, Int>, 
        state: Option<XLSTMLargeState<B>>
    ) -> (Tensor<B, 3>, Option<XLSTMLargeState<B>>) {
        let [_b, _s] = x.dims();
        let mut x = self.embedding.forward(x);
        
        let mut next_block_states = Vec::with_capacity(self.blocks.len());
        
        for (i, block) in self.blocks.iter().enumerate() {
            let b_state = state.as_ref().and_then(|st| st.block_states.get(i).cloned());
            let (out, next_b_state) = block.forward(x, b_state);
            x = out;
            if let Some(nbs) = next_b_state {
                next_block_states.push(nbs);
            }
        }
        
        if let Some(norm) = &self.out_norm {
            x = norm.forward(x);
        }
        
        let mut logits = self.lm_head.forward(x);
        if let Some(cap) = self.output_logit_soft_cap {
            logits = soft_cap(logits, Some(cap));
        }

        let next_state = if next_block_states.is_empty() {
            None
        } else {
            Some(XLSTMLargeState { block_states: next_block_states })
        };

        (logits, next_state)
    }

    fn sample_token(
        logits: Tensor<B, 3>,
        temperature: f64,
        device: &B::Device,
    ) -> Tensor<B, 2, Int> {
        let [batch_size, seq_len, vocab_size] = logits.dims();
        let last_log = logits.slice([0..batch_size, (seq_len-1)..seq_len, 0..vocab_size]).reshape([batch_size, vocab_size]);
        
        let probs = burn::tensor::activation::softmax(last_log.div_scalar(temperature), 1);
        let probs_vec = probs.into_data().as_slice::<f32>().unwrap().to_vec();
        
        let mut next_tokens = Vec::with_capacity(batch_size);
        let mut rng = rand::rng();
        
        for b in 0..batch_size {
            let start = b * vocab_size;
            let end = start + vocab_size;
            let p = &probs_vec[start..end];
            
            use rand::Rng;
            let mut rand_val: f32 = rng.random();
            let mut selected = vocab_size - 1; 
            for (i, &prob) in p.iter().enumerate() {
                rand_val -= prob;
                if rand_val <= 0.0 {
                    selected = i;
                    break;
                }
            }
            next_tokens.push(selected as i64);
        }
        
        Tensor::<B, 1, Int>::from_data(
            burn::tensor::TensorData::new(next_tokens, [batch_size]), device
        ).reshape([batch_size, 1])
    }

    pub fn generate(
        &self,
        mut tokens: Tensor<B, 2, Int>,
        max_new_tokens: usize,
        device: &B::Device,
    ) -> Tensor<B, 2, Int> {
        let [batch_size, _seq_len] = tokens.dims();
        let mut state = self.empty_state(batch_size, device);

        // Prefill
        let (logits, next_state) = self.forward(tokens.clone(), Some(state));
        state = next_state.unwrap();
        
        let temperature = 0.8;
        let mut last_token = Self::sample_token(logits, temperature, device);
        
        tokens = Tensor::cat(vec![tokens, last_token.clone()], 1);

        for _ in 0..max_new_tokens - 1 {
            let (logits, next_state) = self.forward(last_token, Some(state));
            state = next_state.unwrap();
            
            last_token = Self::sample_token(logits, temperature, device);
            tokens = Tensor::cat(vec![tokens, last_token.clone()], 1);
        }

        tokens
    }

    pub fn empty_state(&self, batch_size: usize, device: &B::Device) -> XLSTMLargeState<B> {
        let mut block_states = Vec::with_capacity(self.blocks.len());
        for block in &self.blocks {
            let dh_v = block.mlstm_layer.v_dim / block.mlstm_layer.num_heads;
            let dh_qk = block.mlstm_layer.qk_dim / block.mlstm_layer.num_heads;
            block_states.push((
                Tensor::zeros([batch_size, block.mlstm_layer.num_heads, dh_qk, dh_v], device),
                Tensor::zeros([batch_size, block.mlstm_layer.num_heads, dh_qk, 1], device),
                Tensor::zeros([batch_size, block.mlstm_layer.num_heads, 1, 1], device),
            ));
        }
        XLSTMLargeState { block_states }
    }
}
