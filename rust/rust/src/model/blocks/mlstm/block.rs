// mLSTM Block
// Matches Python: blocks/mlstm/block.py
//
// Just a thin wrapper that delegates to the xLSTM block with mlstm layer.

use burn::config::Config;

use super::layer::MLSTMLayerConfig;

/// Re-export everything since mLSTMBlock is simply configured via xLSTMBlockConfig
/// with mlstm set and slstm/feedforward unset.
/// The actual block logic lives in `blocks::xlstm_block::XLSTMBlock`.

#[derive(Config, Debug)]
pub struct MLSTMBlockConfig {
    pub mlstm: MLSTMLayerConfig,
}
