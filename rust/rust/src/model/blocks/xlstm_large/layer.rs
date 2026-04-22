use burn::prelude::*;
use burn::nn;
use burn::module::Module;

use super::config::XLSTMLargeConfig;
use super::components::{soft_cap, MultiHeadLayerNorm};
use super::backends::MLSTMBackend;

#[derive(Module, Debug)]
pub struct SingleWeights<B: Backend> {
    pub q: nn::Linear<B>,
    pub k: nn::Linear<B>,
    pub v: nn::Linear<B>,
    pub ogate: nn::Linear<B>,
    pub igate: nn::Linear<B>,
    pub fgate: nn::Linear<B>,
}

#[derive(Module, Debug)]
pub struct FusedWeights<B: Backend> {
    pub qkv_ogate: nn::Linear<B>,
    pub ifgate: nn::Linear<B>,
}

#[derive(Module, Debug)]
pub enum WeightMode<B: Backend> {
    Single(SingleWeights<B>),
    Fused(FusedWeights<B>),
}

#[derive(Module, Debug)]
pub struct MLSTMLayer<B: Backend> {
    pub weights: WeightMode<B>,
    pub outnorm: MultiHeadLayerNorm<B>,
    pub out_proj: nn::Linear<B>,
    pub mlstm_backend: MLSTMBackend,
    pub num_heads: usize,
    pub qk_dim: usize,
    pub v_dim: usize,
    pub gate_soft_cap: Option<f64>,
}

impl<B: Backend> MLSTMLayer<B> {
    pub fn init(config: &XLSTMLargeConfig, device: &B::Device) -> Self {
        let qk_dim = config.qk_dim();
        let v_dim = config.v_dim();
        
        let weights = if config.weight_mode == "fused" {
            WeightMode::Fused(FusedWeights {
                qkv_ogate: nn::LinearConfig::new(config.embedding_dim, 2 * qk_dim + 2 * v_dim).with_bias(config.use_bias).init(device),
                ifgate: nn::LinearConfig::new(config.embedding_dim, 2 * config.num_heads).with_bias(true).init(device),
            })
        } else {
            WeightMode::Single(SingleWeights {
                q: nn::LinearConfig::new(config.embedding_dim, qk_dim).with_bias(config.use_bias).init(device),
                k: nn::LinearConfig::new(config.embedding_dim, qk_dim).with_bias(config.use_bias).init(device),
                v: nn::LinearConfig::new(config.embedding_dim, v_dim).with_bias(config.use_bias).init(device),
                ogate: nn::LinearConfig::new(config.embedding_dim, v_dim).with_bias(config.use_bias).init(device),
                igate: nn::LinearConfig::new(config.embedding_dim, config.num_heads).with_bias(true).init(device),
                fgate: nn::LinearConfig::new(config.embedding_dim, config.num_heads).with_bias(true).init(device),
            })
        };

        let outnorm = MultiHeadLayerNorm::init(
            config.num_heads, 
            v_dim / config.num_heads, 
            true, 
            config.use_bias, 
            config.norm_eps, 
            config.norm_reduction_force_float32,
            device
        );
        
        let out_proj = nn::LinearConfig::new(v_dim, config.embedding_dim).with_bias(config.use_bias).init(device);
        
        Self {
            weights,
            outnorm,
            out_proj,
            mlstm_backend: MLSTMBackend::new(config.mlstm_backend.chunk_size, config.mlstm_backend.eps),
            num_heads: config.num_heads,
            qk_dim,
            v_dim,
            gate_soft_cap: config.gate_soft_cap,
        }
    }

    pub fn forward(&self, x: Tensor<B, 3>, state: Option<(Tensor<B, 4>, Tensor<B, 4>, Tensor<B, 4>)>) -> (Tensor<B, 3>, Option<(Tensor<B, 4>, Tensor<B, 4>, Tensor<B, 4>)>) {
        let [b, s, _d] = x.dims();
        
        let (q, k, v, o_preact, i_preact, f_preact) = match &self.weights {
            WeightMode::Fused(fused) => {
                let combined = fused.qkv_ogate.forward(x.clone());
                let q = combined.clone().narrow(2, 0, self.qk_dim);
                let k = combined.clone().narrow(2, self.qk_dim, self.qk_dim);
                let v = combined.clone().narrow(2, 2 * self.qk_dim, self.v_dim);
                let o = combined.narrow(2, 2 * self.qk_dim + self.v_dim, self.v_dim);

                let if_res = soft_cap(fused.ifgate.forward(x), self.gate_soft_cap);
                let i = if_res.clone().narrow(2, 0, self.num_heads);
                let f = if_res.narrow(2, self.num_heads, self.num_heads);
                (q, k, v, o, i, f)
            },
            WeightMode::Single(single) => {
                let q_res = single.q.forward(x.clone());
                let k_res = single.k.forward(x.clone());
                let v_res = single.v.forward(x.clone());
                let o_res = single.ogate.forward(x.clone());
                let i_res = soft_cap(single.igate.forward(x.clone()), self.gate_soft_cap);
                let f_res = soft_cap(single.fgate.forward(x), self.gate_soft_cap);
                (q_res, k_res, v_res, o_res, i_res, f_res)
            }
        };

        let q = q.reshape([b, s, self.num_heads, self.qk_dim / self.num_heads]).swap_dims(1, 2);
        let k = k.reshape([b, s, self.num_heads, self.qk_dim / self.num_heads]).swap_dims(1, 2);
        let v = v.reshape([b, s, self.num_heads, self.v_dim / self.num_heads]).swap_dims(1, 2);
        let i_preact = i_preact.swap_dims(1, 2).unsqueeze_dim(3);
        let f_preact = f_preact.swap_dims(1, 2).unsqueeze_dim(3);

        let (h, next_state) = self.mlstm_backend.forward(q, k, v, i_preact, f_preact, state);
        
        let h = h.swap_dims(1, 2); // (B, S, NH, DH)
        let h_norm = self.outnorm.forward(h); // (B, S, NH * DH)
        
        let og_act = burn::tensor::activation::sigmoid(o_preact);
        let h_out = h_norm * og_act;
        
        (self.out_proj.forward(h_out), next_state)
    }
}
