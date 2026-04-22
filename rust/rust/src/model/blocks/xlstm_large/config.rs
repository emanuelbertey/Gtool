use burn::config::Config;

#[derive(Config, Debug)]
pub struct MLSTMBackendConfig {
    #[config(default = 64)]
    pub chunk_size: usize,
    #[config(default = "1e-6")]
    pub eps: f64,
    #[config(default = "true")]
    pub return_last_states: bool,
    // Note: Kernel types are omitted here as we only have our Rust implementation for now
}

#[derive(Config, Debug)]
pub struct XLSTMLargeConfig {
    pub embedding_dim: usize,
    pub num_heads: usize,
    pub num_blocks: usize,
    pub vocab_size: usize,
    #[config(default = false)]
    pub use_bias: bool,
    #[config(default = 1e-6)]
    pub norm_eps: f64,
    #[config(default = true)]
    pub norm_reduction_force_float32: bool,
    #[config(default = true)]
    pub add_out_norm: bool,
    
    // mlstm layer
    #[config(default = 0.5)]
    pub qk_dim_factor: f64,
    #[config(default = 1.0)]
    pub v_dim_factor: f64,
    
    // backend
    #[config(default = "MLSTMBackendConfig::new()")]
    pub mlstm_backend: MLSTMBackendConfig,
    
    // feedforward
    #[config(default = 2.6667)]
    pub ffn_proj_factor: f64,
    #[config(default = 64)]
    pub ffn_round_up_to_multiple_of: usize,
    
    // capping
    #[config(default = "Some(15.0)")]
    pub gate_soft_cap: Option<f64>,
    #[config(default = "Some(30.0)")]
    pub output_logit_soft_cap: Option<f64>,
    
    // weight mode: "single" or "fused"
    #[config(default = "\"single\".to_string()")]
    pub weight_mode: String,
}

impl XLSTMLargeConfig {
    pub fn qk_dim(&self) -> usize {
        (self.embedding_dim as f64 * self.qk_dim_factor) as usize
    }
    
    pub fn v_dim(&self) -> usize {
        (self.embedding_dim as f64 * self.v_dim_factor) as usize
    }

    pub fn up_proj_dim(&self) -> usize {
        let raw = self.embedding_dim as f64 * self.ffn_proj_factor;
        let multiple = self.ffn_round_up_to_multiple_of as f64;
        let mult = (raw / multiple).ceil() as usize;
        mult * self.ffn_round_up_to_multiple_of
    }
}
