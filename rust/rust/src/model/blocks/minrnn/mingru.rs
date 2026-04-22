use burn::prelude::*;
use burn::tensor::activation;
use burn::tensor::Distribution;
use burn::module::Param;

#[derive(Config, Debug)]
pub struct MinGruConfig {
    pub input_features: usize,
    #[config(default = 2)]
    pub expansion_factor: usize,
}

#[derive(Module, Debug)]
pub struct MinGru<B: Backend> {
    pub linear_z: nn::Linear<B>,
    pub linear_h: nn::Linear<B>,
    pub output_projection: nn::Linear<B>,
}

#[derive(Clone, Debug)]
pub struct MinGruState<B: Backend> {
    pub hidden: Tensor<B, 3>,
}

impl<B: Backend> MinGruState<B> {
    pub fn new(hidden: Tensor<B, 3>) -> Self {
        Self { hidden }
    }
}

impl MinGruConfig {
    pub fn init<B: Backend>(&self, device: &B::Device) -> MinGru<B> {
        let hidden_size = self.input_features * self.expansion_factor;
        
        // Arquitectura idéntica a Python: Sin bias en ninguna capa
        let l_z = nn::LinearConfig::new(self.input_features, hidden_size).with_bias(false).init(device);
        let l_h = nn::LinearConfig::new(self.input_features, hidden_size).with_bias(false).init(device);
        let proj = nn::LinearConfig::new(hidden_size, self.input_features).with_bias(false).init(device);
        
        // Inicialización PyTorch: Uniform(-1/sqrt(in), 1/sqrt(in))
        let init_weights = |linear: nn::Linear<B>, in_dim: usize| {
            let k = (1.0 / in_dim as f32).sqrt();
            let out_dim = linear.weight.dims()[1];
            linear.load_record(nn::LinearRecord {
                weight: Param::from_tensor(Tensor::random([in_dim, out_dim], Distribution::Uniform(-k as f64, k as f64), device)),
                bias: None,
            })
        };

        MinGru { 
            linear_z: init_weights(l_z, self.input_features),
            linear_h: init_weights(l_h, self.input_features),
            output_projection: init_weights(proj, hidden_size) 
        }
    }
}

// Función auxiliar para emular torch.logcumsumexp de forma estable y paralela en f32
fn log_cumsum_exp<B: Backend>(x: Tensor<B, 3>) -> Tensor<B, 3> {
    // 1. Encontrar el máximo a lo largo de la dimensión del tiempo (dim 1)
    // Mantenemos las dimensiones para que el broadcast sea automático (keepdims)
    let max = x.clone().max_dim(1);

    // 2. Aplicar el truco Log-Sum-Exp de forma vectorizada
    // (x - max).exp().cumsum(1).log() + max
    let shifted = x - max.clone();
    let exp_sum = shifted.exp().cumsum(1);
    
    // Añadimos un pequeño epsilon antes del log para evitar log(0) si fuera necesario,
    // aunque con el shift del max, al menos un elemento será exp(0) = 1.
    exp_sum.log() + max
}

fn log_g<B: Backend>(x: Tensor<B, 3>) -> Tensor<B, 3> {
    let mask = x.clone().greater_equal_elem(0.0);
    let pos = (activation::relu(x.clone()) + 0.5).log();
    // Paper: para x < 0, log(sigmoid(x)) = -softplus(-x)
    let neg = activation::softplus(x.neg(), 1.0).neg(); 
    neg.mask_where(mask, pos)
}

fn parallel_scan_log<B: Backend>(log_coeffs: Tensor<B, 3>, log_values: Tensor<B, 3>) -> Tensor<B, 3> {
    let [b, _s_plus_1, h] = log_values.dims();
    let device = log_values.device();
    
    // a_star = cumsum(log_coeffs) con pad zero al inicio para t=0
    let a_star = Tensor::cat(vec![
        Tensor::zeros([b, 1, h], &device),
        log_coeffs.cumsum(1)
    ], 1);
    
    // log_h0_plus_b_star = logcumsumexp(log_values - a_star)
    let x = log_values - a_star.clone();
    let log_h0_plus_b_star = log_cumsum_exp(x);
    
    let log_h = a_star + log_h0_plus_b_star;
    let dims = log_h.dims();
    
    // Regresamos a espacio lineal (h) quitando el estado inicial t=0
    log_h.exp().slice([0..b, 1..dims[1], 0..h])
}

impl<B: Backend> MinGru<B> {
    pub fn forward(&self, x: Tensor<B, 3>, states: Option<Vec<MinGruState<B>>>) -> (Tensor<B, 3>, Vec<MinGruState<B>>) {
        let [b, s, _] = x.dims();
        let device = x.device();
        let hidden_size = self.linear_z.weight.dims()[1];

        let mut states = states.unwrap_or_default();
        let h_prev = states.pop().map(|s| s.hidden);
        let h_0 = h_prev.unwrap_or_else(|| Tensor::zeros([b, 1, hidden_size], &device));

        let k_raw = self.linear_z.forward(x.clone());
        let k = activation::softplus(k_raw, 1.0).neg();
        
        let log_z = activation::softplus(k.clone().neg(), 1.0).neg();
        let log_coeffs = activation::softplus(k, 1.0).neg();
        
        let log_h_0 = log_g(h_0);
        let log_tilde_h = log_g(self.linear_h.forward(x));
        
        let log_values = Tensor::cat(vec![log_h_0, log_z + log_tilde_h], 1);
        let h = parallel_scan_log(log_coeffs, log_values);

        let last_h = h.clone().slice([0..b, s-1..s, 0..hidden_size]);
        (self.output_projection.forward(h), vec![MinGruState::new(last_h)])
    }

    pub fn sequential_mode(&self, x_t: Tensor<B, 3>, h_prev: Tensor<B, 3>) -> (Tensor<B, 3>, Tensor<B, 3>) {
        let k_raw = self.linear_z.forward(x_t.clone());
        let k = activation::softplus(k_raw, 1.0).neg();
        
        // 1. Gating idéntico al espacio logarítmico del forward
        let log_coeffs = activation::softplus(k.clone(), 1.0).neg();
        let log_z = activation::softplus(k.neg(), 1.0).neg();
        
        // 2. Activación de entrada idéntica
        let log_tilde_h = log_g(self.linear_h.forward(x_t));
        
        // 3. Recurrencia: h_t = (1-z)*h_prev + z*tilde_h
        // Usamos exp() para aplicar los valores que el scan calculó en log-space
        let h_t = (log_coeffs.exp() * h_prev) + (log_z + log_tilde_h).exp();

        (self.output_projection.forward(h_t.clone()), h_t)
    }
}
