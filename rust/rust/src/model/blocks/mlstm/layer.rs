// mLSTM Layer
// Matches Python: blocks/mlstm/layer.py
//
// Data flow:
//   x → proj_up → split(x_mlstm, z)
//   x_mlstm → conv1d → SiLU → q_proj, k_proj (from conv output), v_proj (from raw x_mlstm)
//   q, k, v → mLSTM cell → h_tilde
//   h_tilde + learnable_skip * conv_act → h_tilde_skip
//   h_tilde_skip * SiLU(z) → h
//   h → proj_down → output
// mLSTM Layer
// Matches Python: blocks/mlstm/layer.py
//
// Data flow:
//   x → proj_up → split(x_mlstm, z)
//   x_mlstm → conv1d → SiLU → q_proj, k_proj (from conv output), v_proj (from raw x_mlstm)
//   q, k, v → mLSTM cell → h_tilde
//   h_tilde + learnable_skip * conv_act → h_tilde_skip
//   h_tilde_skip * SiLU(z) → h
//   h → proj_down → output
use burn::prelude::*;
use burn::module::{Module, Param};
use burn::config::Config;
use burn::nn;

use crate::components::conv::{CausalConv1d, CausalConv1dConfig};
use crate::components::linear_headwise::{LinearHeadwiseExpand, LinearHeadwiseExpandConfig};
use super::cell::{MLSTMCell, MLSTMCellConfig};

#[derive(Config, Debug)]
pub struct MLSTMLayerConfig {
    pub embedding_dim: usize,
    #[config(default = 4)]
    pub num_heads: usize,
    #[config(default = 4)]
    pub conv1d_kernel_size: usize,
    #[config(default = 4)]
    pub qkv_proj_blocksize: usize,
    #[config(default = 2.0)]
    pub proj_factor: f64,
    #[config(default = false)]
    pub bias: bool,
    #[config(default = 0.0)]
    pub dropout: f64,
    #[config(default = 256)]
    pub context_length: usize,
}

impl MLSTMLayerConfig {
    pub fn inner_embedding_dim(&self) -> usize {
        let raw = self.proj_factor * self.embedding_dim as f64;
        let multiple = 64usize;
        let mult = (raw / multiple as f64).ceil() as usize;
        mult * multiple
    }
}

#[derive(Module, Debug)]
pub struct MLSTMLayer<B: Backend> {
    pub proj_up: nn::Linear<B>,
    pub q_proj: LinearHeadwiseExpand<B>,
    pub k_proj: LinearHeadwiseExpand<B>,
    pub v_proj: LinearHeadwiseExpand<B>,
    pub conv1d: CausalConv1d<B>,
    pub mlstm_cell: MLSTMCell<B>,
    pub learnable_skip: Param<Tensor<B, 1>>,
    pub proj_down: nn::Linear<B>,
    pub dropout: nn::Dropout,
    pub inner_dim: usize,
    pub embedding_dim: usize,
}

impl MLSTMLayerConfig {
    pub fn init<B: Backend>(&self, device: &B::Device) -> MLSTMLayer<B> {
        let inner_dim = self.inner_embedding_dim();
        let num_proj_heads = inner_dim / self.qkv_proj_blocksize;

        let proj_up = nn::LinearConfig::new(self.embedding_dim, 2 * inner_dim)
            .with_bias(self.bias)
            .init(device);

        let q_proj = LinearHeadwiseExpandConfig {
            in_features: inner_dim,
            num_heads: num_proj_heads,
            expand_factor_up: 1.0,
            bias: self.bias,
        }.init(device);

        let k_proj = LinearHeadwiseExpandConfig {
            in_features: inner_dim,
            num_heads: num_proj_heads,
            expand_factor_up: 1.0,
            bias: self.bias,
        }.init(device);

        let v_proj = LinearHeadwiseExpandConfig {
            in_features: inner_dim,
            num_heads: num_proj_heads,
            expand_factor_up: 1.0,
            bias: self.bias,
        }.init(device);

        let conv1d = CausalConv1dConfig {
            feature_dim: inner_dim,
            kernel_size: self.conv1d_kernel_size,
            bias: true,
        }.init(device);

        let mlstm_cell = MLSTMCellConfig {
            context_length: self.context_length,
            embedding_dim: inner_dim,
            num_heads: self.num_heads,
        }.init(device);

        let learnable_skip = Param::from_tensor(Tensor::ones([inner_dim], device));
        let proj_down = nn::LinearConfig::new(inner_dim, self.embedding_dim)
            .with_bias(self.bias)
            .init(device);
        let dropout = nn::DropoutConfig::new(self.dropout).init();

        let mut mlstm_layer = MLSTMLayer {
            proj_up,
            q_proj,
            k_proj,
            v_proj,
            conv1d,
            mlstm_cell,
            learnable_skip,
            proj_down,
            dropout,
            inner_dim,
            embedding_dim: self.embedding_dim,
        };

        // Correct initialization matching Python's mLSTMLayer.reset_parameters()
        mlstm_layer.reset_parameters(self.embedding_dim, 1, device); // Assuming 1 block for layer test

        mlstm_layer
    }
}

impl<B: Backend> MLSTMLayer<B> {
    pub fn reset_parameters(&mut self, embedding_dim: usize, num_blocks: usize, device: &B::Device) {
        use crate::components::init::{small_init_std, wang_init_std};
        
        // 1. Proj Up: small_init_init_
        let proj_up_std = small_init_std(embedding_dim);
        self.proj_up.weight = burn::module::Param::from_tensor(
            Tensor::random(self.proj_up.weight.dims(), burn::tensor::Distribution::Normal(0.0, proj_up_std), device)
        );
        if let Some(bias) = &self.proj_up.bias {
            self.proj_up.bias = Some(burn::module::Param::from_tensor(Tensor::zeros(bias.dims(), device)));
        }

        // 2. QKV Projs: small_init_init_
        let qkv_std = small_init_std(embedding_dim);
        self.q_proj.weight = burn::module::Param::from_tensor(
            Tensor::random(self.q_proj.weight.dims(), burn::tensor::Distribution::Normal(0.0, qkv_std), device)
        );
        self.k_proj.weight = burn::module::Param::from_tensor(
            Tensor::random(self.k_proj.weight.dims(), burn::tensor::Distribution::Normal(0.0, qkv_std), device)
        );
        self.v_proj.weight = burn::module::Param::from_tensor(
            Tensor::random(self.v_proj.weight.dims(), burn::tensor::Distribution::Normal(0.0, qkv_std), device)
        );

        // 3. Proj Down: wang_init_
        let proj_down_std = wang_init_std(embedding_dim, num_blocks);
        self.proj_down.weight = burn::module::Param::from_tensor(
            Tensor::random(self.proj_down.weight.dims(), burn::tensor::Distribution::Normal(0.0, proj_down_std), device)
        );
        if let Some(bias) = &self.proj_down.bias {
            self.proj_down.bias = Some(burn::module::Param::from_tensor(Tensor::zeros(bias.dims(), device)));
        }

        // 4. Skip connection: init to 1.0
        self.learnable_skip = burn::module::Param::from_tensor(Tensor::ones([self.inner_dim], device));

        // 5. MLSTM Cell: Propagate reset_parameters (essential for gates!)
        self.mlstm_cell.reset_parameters(device);
    }
}

pub type MLSTMLayerState<B> = (
    (Tensor<B, 4>, Tensor<B, 4>, Tensor<B, 4>), 
    Tensor<B, 3>,                               
);

impl<B: Backend> MLSTMLayer<B> {
    pub fn empty_state(&self, batch_size: usize, device: &B::Device) -> MLSTMLayerState<B> {
        let dh = self.inner_dim / self.mlstm_cell.num_heads;
        let c = Tensor::zeros([batch_size, self.mlstm_cell.num_heads, dh, dh], device);
        let n = Tensor::zeros([batch_size, self.mlstm_cell.num_heads, dh, 1], device);
        // Estabilizador m inicializado en 0.0 para coincidir con la implementación oficial de Python
        let m = Tensor::zeros([batch_size, self.mlstm_cell.num_heads, 1, 1], device);
        let conv = self.conv1d.empty_state(batch_size, device);
        ((c, n, m), conv)
    }

    pub fn step(
        &self,
        x: Tensor<B, 2>,
        state: MLSTMLayerState<B>,
    ) -> (Tensor<B, 2>, MLSTMLayerState<B>) {
        let (mlstm_state, conv_state) = state;
        let [b, _] = x.dims();

        // 1. Up-projection: (B, D) -> (B, 2*F)
        let x_inner = self.proj_up.forward(x.unsqueeze_dim::<3>(1)).reshape([b, 2 * self.inner_dim]);
        let chunks = x_inner.chunk(2, 1);
        let x_mlstm = chunks[0].clone(); 
        let z = chunks[1].clone(); 

        // 2. Conv + SiLU
        let (x_mlstm_conv, new_conv_state) = self.conv1d.step(x_mlstm.clone(), conv_state);
        let x_mlstm_conv_act = burn::tensor::activation::silu(x_mlstm_conv);

        // 3. Q, K, V Projections
        let q = self.q_proj.forward(x_mlstm_conv_act.clone().unsqueeze_dim::<3>(1));
        let k = self.k_proj.forward(x_mlstm_conv_act.clone().unsqueeze_dim::<3>(1));
        let v = self.v_proj.forward(x_mlstm.unsqueeze_dim::<3>(1));

        // 4. mLSTM Cell Step
        let (h_tilde, new_mlstm_state) = self.mlstm_cell.step(q, k, v, Some(mlstm_state));
        let h_tilde = h_tilde.reshape([b, self.inner_dim]);

        // 5. Learnable Skip Connection (Preservando gradiente del parámetro)
        let skip = self.learnable_skip.val().reshape([1, self.inner_dim]);        
        let h_tilde_skip = h_tilde + x_mlstm_conv_act * skip;

        // 6. Gating & Down-projection
        let z_act = burn::tensor::activation::silu(z);
        let h_state = h_tilde_skip * z_act;

        let y = self.proj_down.forward(h_state.unsqueeze_dim::<3>(1)).reshape([b, self.embedding_dim]);
        let y = self.dropout.forward(y.unsqueeze_dim::<3>(1)).reshape([b, self.embedding_dim]);

        (y, (new_mlstm_state, new_conv_state))
    }

    pub fn forward(&self, x: Tensor<B, 3>) -> Tensor<B, 3> {
        let x_inner = self.proj_up.forward(x);
        let chunks = x_inner.chunk(2, 2);
        let x_mlstm = chunks[0].clone();
        let z = chunks[1].clone();

        let x_mlstm_conv = self.conv1d.forward(x_mlstm.clone());
        let x_mlstm_conv_act = burn::tensor::activation::silu(x_mlstm_conv);

        let q = self.q_proj.forward(x_mlstm_conv_act.clone());
        let k = self.k_proj.forward(x_mlstm_conv_act.clone());
        let v = self.v_proj.forward(x_mlstm.clone());

        let h_tilde = self.mlstm_cell.forward(q, k, v);

        // Skip connection con broadcasting consistente
        let skip = self.learnable_skip.val().reshape([1, 1, self.inner_dim]);
        let h_tilde_skip = h_tilde + x_mlstm_conv_act * skip;

        let z_act = burn::tensor::activation::silu(z);
        let h_state = h_tilde_skip * z_act;

        self.dropout.forward(self.proj_down.forward(h_state))
    }
}
