use godot::prelude::*;
use burn::prelude::*;
use burn::record::{CompactRecorder, Recorder};
use burn_ndarray::NdArray;
use burn::tensor::activation::softmax;
use burn::tensor::TensorData;
use burn::optim::{AdamConfig, Optimizer, GradientsParams};
use burn::optim::decay::WeightDecayConfig;
use burn::nn::loss::CrossEntropyLossConfig;
use burn::grad_clipping::GradientClippingConfig;
use burn::module::AutodiffModule;
use burn_autodiff::Autodiff;

use std::error::Error;
use std::path::Path;
use std::fs;
use std::io::{self, Read, BufReader};

use tokenizers::AddedToken;
use tokenizers::decoders::metaspace::Metaspace as MetaspaceDecoder;
use tokenizers::models::bpe::{BpeTrainerBuilder, BPE};
use tokenizers::pre_tokenizers::metaspace::{Metaspace, PrependScheme};
use tokenizers::tokenizer::Tokenizer as HFTokenizer;
use tokenizers::models::TrainerWrapper;

use xlstm::blocks::xlstm_large::{XLSTMLarge, XLSTMLargeConfig};

// We use Autodiff for training support, NdArray for CPU
type MyBackend = Autodiff<NdArray<f32>>;

/// Professional Tokenizer using Hugging Face 'tokenizers'
pub struct LocalTokenizer {
    tokenizer: HFTokenizer,
}

impl LocalTokenizer {
    pub fn load(path: &str) -> Result<Self, Box<dyn Error>> {
        let mut tokenizer = HFTokenizer::from_file(path).map_err(|e| format!("{}", e))?;
        tokenizer.with_decoder(Some(MetaspaceDecoder::new('▁', PrependScheme::Always, true)));
        Ok(Self { tokenizer })
    }

    pub fn from_text(text: &str, vocab_size: usize) -> Result<Self, Box<dyn Error>> {
        let model = BPE::builder()
            .byte_fallback(true)
            .build()
            .map_err(|e| format!("Error building BPE: {}", e))?;
            
        let mut tokenizer = HFTokenizer::new(model);
        tokenizer.with_pre_tokenizer(Some(Metaspace::new('▁', PrependScheme::Always, true)));
        tokenizer.with_decoder(Some(MetaspaceDecoder::new('▁', PrependScheme::Always, true)));

        let special_token = "<|endoftext|>";
        tokenizer.add_special_tokens(&[AddedToken::from(special_token.to_string(), true)]);

        let trainer = BpeTrainerBuilder::default()
            .show_progress(true)
            .vocab_size(vocab_size)
            .min_frequency(2)
            .special_tokens(vec![
                AddedToken::from(special_token.to_string(), true)
            ])
            .build();

        let mut trainer_wrapper = TrainerWrapper::from(trainer);
        let temp_file = "temp_train_large_godot.txt";
        fs::write(temp_file, text)?;
        tokenizer.train_from_files(&mut trainer_wrapper, vec![temp_file.to_string()])
            .map_err(|e| format!("Error en entrenamiento: {}", e))?;
        let _ = fs::remove_file(temp_file);

        Ok(Self { tokenizer })
    }

    pub fn save(&self, path: &str) -> Result<(), Box<dyn Error>> {
        self.tokenizer.save(path, true).map_err(|e| format!("{}", e))?;
        Ok(())
    }

    pub fn encode(&self, text: &str) -> Vec<usize> {
        let encoding = self.tokenizer.encode(text, false).unwrap();
        encoding.get_ids().iter().map(|&id| id as usize).collect()
    }

    pub fn decode(&self, indices: &[usize]) -> String {
        let u32_indices: Vec<u32> = indices.iter().map(|&idx| idx as u32).collect();
        self.tokenizer.decode(&u32_indices, true).unwrap()
    }

    pub fn vocab_size(&self) -> usize {
        self.tokenizer.get_vocab_size(true)
    }

    pub fn id_to_token(&self, id: usize) -> Option<String> {
        self.tokenizer.id_to_token(id as u32)
    }
}

struct FileFragmentIterator {
    reader: BufReader<fs::File>,
    buffer_size: usize,
    finished: bool,
}

impl FileFragmentIterator {
    fn new(path: &Path, buffer_size_mb: usize) -> io::Result<Self> {
        let file = fs::File::open(path)?;
        Ok(Self {
            reader: BufReader::new(file),
            buffer_size: buffer_size_mb * 1024 * 1024,
            finished: false,
        })
    }
}

impl Iterator for FileFragmentIterator {
    type Item = String;
    fn next(&mut self) -> Option<Self::Item> {
        if self.finished { return None; }
        let mut buffer = vec![0u8; self.buffer_size];
        let mut total_read = 0;
        while total_read < self.buffer_size {
            match self.reader.read(&mut buffer[total_read..]) {
                Ok(0) => { self.finished = true; break; }
                Ok(n) => total_read += n,
                _ => { self.finished = true; break; }
            }
        }
        if total_read == 0 { return None; }
        buffer.truncate(total_read);
        Some(String::from_utf8_lossy(&buffer).into_owned())
    }
}

#[derive(GodotClass)]
#[class(base=Node)]
pub struct XLSTMLargeChat {
    // Current state
    model: Option<XLSTMLarge<MyBackend>>,
    tokenizer: Option<LocalTokenizer>,
    device: <MyBackend as Backend>::Device,
    
    // Options (Matching large_chat.rs exactly)
    #[export] pub num_blocks: i32,
    #[export] pub num_heads: i32,
    #[export] pub lr: f64,
    #[export] pub num_epochs: i32,
    #[export] pub batch_size: i32,
    #[export] pub temperature: f32,
    #[export] pub repetition_penalty: f32,
    #[export] pub embedding_dim: i32,
    #[export] pub mode: GString, // "single" or "fused"
    #[export] pub model_file: GString,
    #[export] pub target_vocab_size: i32,
    #[export] pub tokenizer_path: GString,
    #[export] pub seq_length: i32,
    
    base: Base<Node>,
}

#[godot_api]
impl INode for XLSTMLargeChat {
    fn init(base: Base<Node>) -> Self {
        Self {
            model: None,
            tokenizer: None,
            device: Default::default(),
            
            num_blocks: 2,
            num_heads: 4,
            lr: 3e-3,
            num_epochs: 20,
            batch_size: 16,
            temperature: 0.8,
            repetition_penalty: 1.1,
            embedding_dim: 256,
            mode: "single".into(),
            model_file: "large_model.mpk".into(),
            target_vocab_size: 1024,
            tokenizer_path: "tokenizer.json".into(),
            seq_length: 256,
            
            base,
        }
    }
}

#[godot_api]
impl XLSTMLargeChat {
    #[func]
    pub fn init_session(&mut self, text_file_for_tokenizer: GString) -> bool {
        let t_path = self.tokenizer_path.to_string();
        
        // 1. Load or train tokenizer
        let tokenizer = if Path::new(&t_path).exists() {
            match LocalTokenizer::load(&t_path) {
                Ok(t) => t,
                Err(e) => {
                    godot_error!("Failed to load tokenizer: {}", e);
                    return false;
                }
            }
        } else {
            let path = text_file_for_tokenizer.to_string();
            if path.is_empty() || !Path::new(&path).exists() {
                godot_error!("Tokenizer file not found and no training file provided.");
                return false;
            }
            let text = fs::read_to_string(&path).unwrap_or_default();
            match LocalTokenizer::from_text(&text, self.target_vocab_size as usize) {
                Ok(t) => {
                    let _ = t.save(&t_path);
                    t
                },
                Err(e) => {
                    godot_error!("Failed to train tokenizer: {}", e);
                    return false;
                }
            }
        };

        let vocab_size = tokenizer.vocab_size();
        
        // 2. Config
        let config = XLSTMLargeConfig {
            embedding_dim: self.embedding_dim as usize,
            num_heads: self.num_heads as usize,
            num_blocks: self.num_blocks as usize,
            vocab_size,
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
            weight_mode: self.mode.to_string(),
        };

        // 3. Model
        let m_path = self.model_file.to_string();
        let model = if Path::new(&m_path).exists() {
            let recorder = CompactRecorder::new();
            match recorder.load(m_path.into(), &self.device) {
                Ok(record) => XLSTMLarge::init(&config, &self.device).load_record(record),
                Err(_) => {
                    godot_print!("Warning: Failed to load model record, initiating new model.");
                    XLSTMLarge::init(&config, &self.device)
                }
            }
        } else {
            XLSTMLarge::init(&config, &self.device)
        };

        self.tokenizer = Some(tokenizer);
        self.model = Some(model);
        
        godot_print!("XLSTMLarge session initialized. Vocab: {}", vocab_size);
        true
    }

    #[func]
    pub fn save_model(&self, path_override: GString) -> bool {
        let path = if path_override.is_empty() { self.model_file.to_string() } else { path_override.to_string() };
        if let Some(model) = &self.model {
            let recorder = CompactRecorder::new();
            if let Err(e) = model.clone().save_file(path.clone(), &recorder) {
                godot_error!("Failed to save model: {}", e);
                return false;
            }
            godot_print!("Model saved to {}", path);
            return true;
        }
        false
    }

    #[func]
    pub fn train_on_file(&mut self, text_file: GString) -> bool {
        let (mut model, tokenizer) = match (self.model.take(), &self.tokenizer) {
            (Some(m), Some(t)) => (m, t),
            _ => { godot_error!("Session not initialized."); return false; }
        };

        let mut optim = AdamConfig::new()
            .with_weight_decay(Some(WeightDecayConfig::new(1e-5)))
            .with_grad_clipping(Some(GradientClippingConfig::Norm(0.5)))
            .init();

        let loss_fn = CrossEntropyLossConfig::new().init(&self.device);
        let tokens_per_batch = (self.batch_size * self.seq_length) as usize;

        for epoch in 0..self.num_epochs {
            let mut total_loss = 0.0;
            let mut batch_count = 0;
            let fragments = match FileFragmentIterator::new(Path::new(&text_file.to_string()), 1) {
                Ok(f) => f,
                Err(e) => { godot_error!("Failed to open file: {}", e); break; }
            };

            for fragment in fragments {
                let tokens = tokenizer.encode(&fragment);
                let num_batches = tokens.len() / tokens_per_batch;
                if num_batches == 0 { continue; }

                for batch_idx in 0..num_batches {
                    let start_idx = batch_idx * tokens_per_batch;
                    let (input, targets) = create_batch::<MyBackend>(&tokens, start_idx, self.batch_size as usize, self.seq_length as usize, &self.device);

                    let (logits, _) = model.forward(input, None);
                    let [b, s, v] = logits.dims();
                    let loss = loss_fn.forward(logits.reshape([b * s, v]), targets.reshape([b * s]));
                    
                    total_loss += loss.clone().into_data().as_slice::<f32>().unwrap()[0];
                    batch_count += 1;

                    let grads = loss.backward();
                    let grads = GradientsParams::from_grads(grads, &model);
                    model = optim.step(self.lr, model, grads);

                    if batch_idx % 10 == 0 {
                        godot_print!("Epoch {}/{} | Batch {}/{} | Loss: {:.4}", epoch+1, self.num_epochs, batch_idx+1, num_batches, total_loss / batch_count as f32);
                    }
                }
            }
            godot_print!("Epoch {} completed. Avg Loss: {:.4}", epoch+1, total_loss / batch_count as f32);
        }

        self.model = Some(model);
        true
    }

    #[func]
    pub fn generate(&self, seed_text: GString, length: i32) -> GString {
        let (model, tokenizer) = match (&self.model, &self.tokenizer) {
            (Some(m), Some(t)) => (m, t),
            _ => return "".into()
        };

        let seed_ids = tokenizer.encode(&seed_text.to_string());
        if seed_ids.is_empty() { return "".into(); }

        let mut current_state = model.valid().empty_state(1, &self.device);
        let input = Tensor::<MyBackend, 2, Int>::from_data(
            TensorData::new(seed_ids.iter().map(|&id| id as i64).collect(), [1, seed_ids.len()]), 
            &self.device
        );
        
        let (logits, next_state) = model.valid().forward(input.valid(), None);
        current_state = next_state.expect("State error");

        let [_, s_len, v_dim] = logits.dims();
        let mut last_logits = logits.slice([0..1, (s_len - 1)..s_len]).reshape([1, v_dim]);

        let mut result_ids = Vec::new();
        let mut history = seed_ids.clone();

        let mut next_id = sample_from_logits(last_logits, self.temperature, 20, 1.0, self.repetition_penalty, &history);

        for _ in 0..length {
            if let Some(token) = tokenizer.id_to_token(next_id) {
                if token == "<|endoftext|>" { break; }
            }

            result_ids.push(next_id);
            history.push(next_id);
            if history.len() > 64 { history.remove(0); }

            let input = Tensor::<MyBackend, 2, Int>::from_data(TensorData::new(vec![next_id as i64], [1, 1]), &self.device);
            let (logits, next_state) = model.valid().forward(input.valid(), Some(current_state));
            current_state = next_state.expect("State error");
            
            let [_, _, v] = logits.dims();
            next_id = sample_from_logits(logits.reshape([1, v]), self.temperature, 20, 1.0, self.repetition_penalty, &history);
        }

        GString::from(&tokenizer.decode(&result_ids))
    }
}

fn create_batch<B: Backend>(
    tokens: &[usize],
    start_idx: usize,
    batch_size: usize,
    seq_length: usize,
    device: &B::Device,
) -> (Tensor<B, 2, Int>, Tensor<B, 2, Int>) {
    let mut x_indices = Vec::with_capacity(batch_size * seq_length);
    let mut y_indices = Vec::with_capacity(batch_size * seq_length);

    for i in 0..batch_size {
        let current_start = start_idx + i * seq_length;
        for j in 0..seq_length {
            let x_idx = if current_start + j < tokens.len() { tokens[current_start + j] } else { 0 };
            let y_idx = if current_start + j + 1 < tokens.len() { tokens[current_start + j + 1] } else { 0 };
            x_indices.push(x_idx as i64);
            y_indices.push(y_idx as i64);
        }
    }

    let x = Tensor::<B, 2, Int>::from_data(TensorData::new(x_indices, [batch_size, seq_length]), device);
    let y = Tensor::<B, 2, Int>::from_data(TensorData::new(y_indices, [batch_size, seq_length]), device);
    (x, y)
}

fn sample_from_logits<B: Backend>(
    logits: Tensor<B, 2>, 
    temperature: f32,
    top_k: usize,
    top_p: f32,
    repetition_penalty: f32,
    previous_tokens: &[usize],
) -> usize {
    let probs = softmax(logits, 1);
    let mut probs_vec: Vec<(usize, f32)> = probs.into_data()
        .as_slice::<f32>()
        .unwrap()
        .iter()
        .enumerate()
        .map(|(i, &x)| (i, x))
        .collect();

    if repetition_penalty != 1.0 {
        for (id, prob) in probs_vec.iter_mut() {
            if previous_tokens.contains(id) {
                *prob /= repetition_penalty;
            }
        }
    }

    probs_vec.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    
    let k = top_k.min(probs_vec.len()).max(1);
    let mut filtered_probs = Vec::with_capacity(k);
    let mut cumulative_prob = 0.0;
    for (i, p) in probs_vec.into_iter() {
        filtered_probs.push((i, p));
        cumulative_prob += p;
        if filtered_probs.len() >= k || cumulative_prob >= top_p {
            break;
        }
    }

    let indices: Vec<usize> = filtered_probs.iter().map(|(i, _)| *i).collect();
    let mut weights: Vec<f32> = filtered_probs.iter().map(|(_, p)| *p).collect();

    if temperature <= 1e-6 {
        return indices[0];
    }

    for p in weights.iter_mut() {
        *p = (p.max(1e-10).ln() / temperature).exp();
    }

    let sum: f32 = weights.iter().sum();
    use rand::Rng;
    let mut rng = rand::rng(); 
    let sample: f32 = rng.random::<f32>() * sum; 
    let mut cumulative = 0.0;
    for (i, &p) in weights.iter().enumerate() {
        cumulative += p;
        if sample <= cumulative {
            return indices[i];
        }
    }
    indices[0]
}
