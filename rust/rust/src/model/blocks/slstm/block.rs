// sLSTM Block config
// Matches Python: blocks/slstm/block.py

use burn::config::Config;
use super::layer::SLSTMLayerConfig;
use crate::components::feedforward::GatedFeedForwardConfig;

#[derive(Config, Debug)]
pub struct SLSTMBlockConfig {
    pub slstm: SLSTMLayerConfig,
    pub feedforward: Option<GatedFeedForwardConfig>,
}
