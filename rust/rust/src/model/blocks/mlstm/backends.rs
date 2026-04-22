use burn::prelude::*;

pub fn parallel_stabilized_simple<B: Backend>(
    queries: Tensor<B, 4>,
    keys: Tensor<B, 4>,
    values: Tensor<B, 4>,
    igate_preact: Tensor<B, 4>,
    fgate_preact: Tensor<B, 4>,
) -> Tensor<B, 4> {
    let [_b, _nh, s, dh] = queries.dims();
    let device = queries.device();
    let eps: f64 = 1e-6;
    let scale = 1.0 / (dh as f64).sqrt();

    // 1. Gates y cumsum
    let log_fg = log_sigmoid(fgate_preact);
    let log_fg_cumsum = cumsum_matrix::<B>(log_fg, s, &device);
    
    // 2. Matriz de decaimiento
    let log_fg_matrix = log_fg_cumsum.clone() - log_fg_cumsum.swap_dims(2, 3);
    
    // 3. Estabilización con Input Gate
    let log_d_matrix = log_fg_matrix + igate_preact.swap_dims(2, 3);

    // 4. Máscara Causal (S, S) -> (1, 1, S, S)
    let mask = Tensor::<B, 2>::ones([s, s], &device)
        .tril(0)
        .unsqueeze::<3>()
        .unsqueeze::<4>();
    
    // m_i = max_{j <= i} (log_D_{ij})
    // Usamos NEG_INFINITY para que exp() del futuro sea 0 absoluto, igual que en Python.
    let masked_log_d = log_d_matrix.mask_fill(mask.clone().equal_elem(0.0), f32::NEG_INFINITY);
    let m = masked_log_d.clone().max_dim(3);

    // 5. Matriz de atención estabilizada
    let d_matrix = (masked_log_d - m.clone()).exp().mask_fill(mask.equal_elem(0.0), 0.0);

    // 6. Cómputo final
    // python: qk_matrix = queries @ keys_scaled.transpose(-2, -1)
    let keys_scaled = keys * scale;
    let qk_matrix = queries.matmul(keys_scaled.swap_dims(2, 3));
    let c_matrix = qk_matrix * d_matrix;
    
    let qn_dot = c_matrix.clone().sum_dim(3).abs();
    // Safety clamp (opcional si hay NaNs en CPU): let m_safe = m.neg().clamp_max(88.0);
    // let h_den = qn_dot.max_pair(m_safe.exp()) + eps; 
    let h_den = qn_dot.max_pair(m.neg().exp()) + eps;

    // python: C_matrix_normalized = C_matrix / (normalizer + eps)
    // h_tilde_state = C_matrix_normalized @ values
    let c_matrix_normalized = c_matrix / h_den;
    c_matrix_normalized.matmul(values)
}

pub fn recurrent_step_stabilized_simple<B: Backend>(
    c_state: Tensor<B, 4>,
    n_state: Tensor<B, 4>,
    m_state: Tensor<B, 4>,
    q: Tensor<B, 4>,
    k: Tensor<B, 4>,
    v: Tensor<B, 4>,
    igate_preact: Tensor<B, 4>,
    fgate_preact: Tensor<B, 4>,
) -> (Tensor<B, 4>, (Tensor<B, 4>, Tensor<B, 4>, Tensor<B, 4>)) {
    let dh = q.dims()[3];
    let eps: f64 = 1e-6;
    let scale = 1.0 / (dh as f64).sqrt();

    let log_fg = log_sigmoid(fgate_preact); 
    
    // Evitamos moves clonando los estados que se usan varias veces
    let m_new = (m_state.clone() + log_fg.clone()).max_pair(igate_preact.clone()); 

    let f_act = (m_state + log_fg - m_new.clone()).exp();
    let i_act = (igate_preact - m_new.clone()).exp(); 

    // python: k_scaled = k / math.sqrt(DH)
    let k_scaled = k * scale;
    let k_t_scaled = k_scaled.swap_dims(2, 3); 

    // python: c_state_new = fg_act * c_state + ig_act * (k_scaled @ v.transpose(-1, -2))
    let c_new = c_state * f_act.clone() + k_t_scaled.clone().matmul(v) * i_act.clone();
    
    // python: n_state_new = fg_act * n_state + ig_act * k_scaled
    let n_new = n_state * f_act + k_t_scaled * i_act;

    let h_num = q.clone().matmul(c_new.clone()); 
    let qn_dot = q.matmul(n_new.clone());

    // Safety clamp (opcional si hay NaNs en CPU): let m_safe = m_new.clone().neg().clamp_max(88.0);
    // let h_denom = qn_dot.abs().max_pair(m_safe.exp()) + eps;
    let h_denom = qn_dot.abs().max_pair(m_new.clone().neg().exp()) + eps;

    (h_num / h_denom, (c_new, n_new, m_new))
}

fn log_sigmoid<B: Backend, const D: usize>(x: Tensor<B, D>) -> Tensor<B, D> {
    burn::tensor::activation::log_sigmoid(x)
}

fn cumsum_matrix<B: Backend>(x: Tensor<B, 4>, _s: usize, _device: &B::Device) -> Tensor<B, 4> {
    x.cumsum(2)
}