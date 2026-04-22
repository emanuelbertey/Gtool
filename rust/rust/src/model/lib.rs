pub mod components;
pub mod blocks;
pub mod xlstm_block_stack;
pub mod xlstm_lm_model;

// Re-exports for easier access matching the old API expectations mapping to the new port
pub use xlstm_lm_model::{XLSTMLMModel as XLstm, XLSTMLMModelConfig as XLstmConfig};
pub use xlstm_block_stack::{XLSTMBlockStackState as XLstmState};
pub use blocks::minrnn::mingru::{MinGru, MinGruConfig, MinGruState};
pub use blocks::minrnn::minlstm::{MinLstm, MinLstmConfig, MinLstmState};
pub use blocks::slstm::layer::{SLSTMLayer as SLstm, SLSTMLayerConfig as SLstmConfig};
pub use blocks::slstm::cell::{SLSTMState as SLstmState};

#[derive(Debug, Clone, Copy)]
pub enum LstmType {
    MLSTM,
    SLSTM,
    Alternate,
    MINGRU,
}

#[derive(Debug, Clone)]
pub struct LearningRateConfig {
    pub slstm_lr: f64,
    pub mlstm_lr: f64,
    pub other_lr: f64,
}

impl LearningRateConfig {
    pub fn per_block_type(slstm_lr: f64, mlstm_lr: f64, _un: f64, other_lr: f64) -> Self {
        Self {
            slstm_lr,
            mlstm_lr,
            other_lr,
        }
    }
}
