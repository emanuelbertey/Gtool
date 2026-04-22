// xLSTM demo — builds and runs a simple xLSTM language model

use burn::backend::NdArray;
use burn::prelude::*;

use xlstm::blocks::mlstm::block::MLSTMBlockConfig;
use xlstm::blocks::mlstm::layer::MLSTMLayerConfig;
use xlstm::blocks::slstm::block::SLSTMBlockConfig;
use xlstm::blocks::slstm::layer::SLSTMLayerConfig;
use xlstm::components::feedforward::GatedFeedForwardConfig;
use xlstm::xlstm_block_stack::XLSTMBlockStackConfig;
use xlstm::xlstm_lm_model::{XLSTMLMModel, XLSTMLMModelConfig};

type MyBackend = NdArray<f32>;

fn main() {
    let device = Default::default();

    // ── Configure a small xLSTM language model ──────────────────────────
    let embedding_dim = 64;
    let vocab_size = 256;
    let num_blocks = 4;

    let mlstm_layer_cfg = MLSTMLayerConfig {
        embedding_dim, // will be overridden by block_stack
        num_heads: 4,
        conv1d_kernel_size: 4,
        qkv_proj_blocksize: 4,
        proj_factor: 2.0,
        bias: false,
        dropout: 0.0,
        context_length: 64,
    };

    let slstm_layer_cfg = SLSTMLayerConfig {
        embedding_dim,
        num_heads: 4,
        conv1d_kernel_size: 4,
        dropout: 0.0,
    };

    let slstm_ff_cfg = GatedFeedForwardConfig {
        embedding_dim,
        proj_factor: 1.3,
        bias: false,
        dropout: 0.0,
    };

    let block_stack_cfg = XLSTMBlockStackConfig {
        embedding_dim,
        num_blocks,
        context_length: 64,
        add_post_blocks_norm: true,
        bias: false,
        dropout: 0.0,
        mlstm_block: Some(MLSTMBlockConfig {
            mlstm: mlstm_layer_cfg,
        }),
        slstm_block: Some(SLSTMBlockConfig {
            slstm: slstm_layer_cfg,
            feedforward: Some(slstm_ff_cfg),
        }),
        // blocks 1 and 3 are sLSTM, 0 and 2 are mLSTM
        slstm_at: vec![1, 3],
    };

    let model_cfg = XLSTMLMModelConfig {
        vocab_size,
        add_embedding_dropout: false,
        block_stack: block_stack_cfg,
    };

    // ── Build the model ─────────────────────────────────────────────────
    println!("Building xLSTM LM model...");
    println!("  Embedding dim: {}", embedding_dim);
    println!("  Vocab size: {}", vocab_size);
    println!("  Num blocks: {}", num_blocks);
    println!("  Block map: [mLSTM, sLSTM, mLSTM, sLSTM]");

    let model: XLSTMLMModel<MyBackend> = model_cfg.init(&device);

    // ── Forward pass ────────────────────────────────────────────────────
    let batch_size = 2;
    let seq_len = 16;
    let input_data: Vec<i32> = (0..batch_size * seq_len)
        .map(|i| i % vocab_size as i32)
        .collect();
    let input = Tensor::<MyBackend, 1, Int>::from_ints(input_data.as_slice(), &device)
        .reshape([batch_size, seq_len]);

    println!("\nRunning forward pass with input shape [{}, {}]...", batch_size, seq_len);
    let logits = model.forward(input);
    let [b, s, v] = logits.dims();
    println!("Output logits shape: [{}, {}, {}]", b, s, v);
    println!("\n✓ xLSTM model built and ran successfully!");

    // ── Configure a small xLSTMLarge model ──────────────────────────────
    println!("\n--- Testing xLSTMLarge ---");
    use xlstm::blocks::xlstm_large::XLSTMLargeConfig;
    use xlstm::blocks::xlstm_large::model::XLSTMLarge;

    let large_config = XLSTMLargeConfig {
        embedding_dim: 64,
        num_heads: 4,
        num_blocks: 2,
        vocab_size: 256,
        use_bias: true,
        norm_eps: 1e-6,
        norm_reduction_force_float32: true,
        add_out_norm: true,
        qk_dim_factor: 0.5,
        v_dim_factor: 1.0,
        mlstm_backend: xlstm::blocks::xlstm_large::config::MLSTMBackendConfig::new(),
        ffn_proj_factor: 2.6667,
        ffn_round_up_to_multiple_of: 64,
        gate_soft_cap: Some(15.0),
        output_logit_soft_cap: Some(30.0),
        weight_mode: "single".to_string(),
    };

    println!("Building xLSTMLarge model...");
    let large_model: XLSTMLarge<MyBackend> = XLSTMLarge::init(&large_config, &device);

    let input_large_data: Vec<i32> = (0..batch_size * seq_len)
        .map(|i| i % vocab_size as i32)
        .collect();
    let input_large = Tensor::<MyBackend, 1, Int>::from_ints(input_large_data.as_slice(), &device)
        .reshape([batch_size, seq_len]);

    println!("Running forward pass for xLSTMLarge...");
    let (logits_large, _) = large_model.forward(input_large, None);
    let [bl, sl, vl] = logits_large.dims();
    println!("xLSTMLarge Output logits shape: [{}, {}, {}]", bl, sl, vl);
    println!("✓ xLSTMLarge model built and ran successfully!");
}
