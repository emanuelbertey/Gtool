use burn::prelude::*;
use burn::module::Module;

#[derive(Module, Debug, Clone)]
pub struct MLSTMBackend {
    pub chunk_size: usize,
    pub eps: f64,
}

impl MLSTMBackend {
    pub fn new(chunk_size: usize, eps: f64) -> Self {
        Self { chunk_size, eps }
    }

    pub fn forward<B: Backend>(
        &self,
        q: Tensor<B, 4>,
        k: Tensor<B, 4>,
        v: Tensor<B, 4>,
        i: Tensor<B, 4>,
        f: Tensor<B, 4>,
        state: Option<(Tensor<B, 4>, Tensor<B, 4>, Tensor<B, 4>)>,
    ) -> (Tensor<B, 4>, Option<(Tensor<B, 4>, Tensor<B, 4>, Tensor<B, 4>)>) {
        let [_b, _nh, s, _dh] = q.dims();
        let device = q.device();

        if s > 1 && state.is_none() {
            // --- PARALLEL PREFILL ---
            // Process the whole prompt at once (efficient O(S^2) on GPU).
            // Now computes and returns the last state so we can continue with steps.
            parallel_stabilized_simple(q, k, v, i, f, self.eps)
        } else {
            // --- RECURRENT GENERATION ---
            // Used for step-by-step generation or if an initial state is provided.
            let mut current_state = state.unwrap_or_else(|| {
                let [b, nh, _, dh_qk] = q.dims();
                let dh_v = v.dims()[3];
                (
                    Tensor::zeros([b, nh, dh_qk, dh_v], &device),
                    Tensor::zeros([b, nh, dh_qk, 1], &device),
                    Tensor::zeros([b, nh, 1, 1], &device),
                )
            });

            let mut outs = Vec::with_capacity(s);
            for t in 0..s {
                let qt = q.clone().narrow(2, t, 1);
                let kt = k.clone().narrow(2, t, 1);
                let vt = v.clone().narrow(2, t, 1);
                let it = i.clone().narrow(2, t, 1);
                let ft = f.clone().narrow(2, t, 1);

                let (h, next_state) = recurrent_step_stabilized_simple(
                    current_state.0,
                    current_state.1,
                    current_state.2,
                    qt,
                    kt,
                    vt,
                    it,
                    ft,
                    self.eps,
                );
                current_state = next_state;
                outs.push(h);
            }
            let y = Tensor::cat(outs, 2);
            (y, Some(current_state))
        }
    }
}

/// Parallel (quadratic) stabilized mLSTM forward pass.
///
/// Matches `parallel_stabilized_simple` from `xlstm/blocks/mlstm/backends.py` exactly:
///
/// 1. Forget-gate cumsum with a prepended zero column (shape S+1), subtracted and
///    then sliced [1:, 1:] — this is identical to the Python reference.
/// 2. Causal mask uses true `-inf` (f32::NEG_INFINITY) instead of a large finite
///    negative number, guaranteeing exp(-inf) == 0.0 in all dtypes including bfloat16.
///
/// Args (all `(B, NH, S, DH)` or `(B, NH, S, 1)` for gates):
///   queries, keys, values — (B, NH, S, DH)
///   igate_preact, fgate_preact — (B, NH, S, 1)  pre-activations
///   eps_val — small epsilon for numerical stability
///
/// Returns: h_tilde — (B, NH, S, DH)
pub fn parallel_stabilized_simple<B: Backend>(
    queries: Tensor<B, 4>,
    keys: Tensor<B, 4>,
    values: Tensor<B, 4>,
    igate_preact: Tensor<B, 4>,
    fgate_preact: Tensor<B, 4>,
    eps_val: f64,
) -> (Tensor<B, 4>, Option<(Tensor<B, 4>, Tensor<B, 4>, Tensor<B, 4>)>) {
    let [b, nh, s, dh] = queries.dims();
    let device = queries.device();
    let scale = 1.0 / (dh as f64).sqrt();

    let log_fg = burn::tensor::activation::log_sigmoid(fgate_preact);

    // Cumsum with zero prefix (S+1)
    let zero_prefix = Tensor::<B, 4>::zeros([b, nh, 1, 1], &device);
    let log_fg_cumsum = Tensor::cat(vec![zero_prefix, log_fg.cumsum(2)], 2); // (B, NH, S+1, 1)

    // Decay matrix D[i,j] = exp(sum_{l=j+1}^{i} log f_l + log i_j)
    let log_fg_matrix_full = log_fg_cumsum.clone() - log_fg_cumsum.clone().swap_dims(2, 3);
    let log_fg_matrix = log_fg_matrix_full.narrow(2, 1, s).narrow(3, 1, s);
    let log_d_matrix = log_fg_matrix + igate_preact.clone().swap_dims(2, 3);

    let causal_mask = Tensor::<B, 2>::ones([s, s], &device).tril(0).reshape([1, 1, s, s]);
    let masked_log_d = log_d_matrix.mask_fill(causal_mask.clone().equal_elem(0.0), f32::NEG_INFINITY as f64);

    let m = masked_log_d.clone().max_dim(3);
    let d_matrix = (masked_log_d - m.clone()).exp().mask_fill(causal_mask.equal_elem(0.0), 0.0);

    let qk_matrix = queries.clone().matmul(keys.clone().swap_dims(2, 3)) * scale;
    let c_matrix = qk_matrix * d_matrix;

    let normalizer = c_matrix.clone().sum_dim(3).abs().max_pair(m.neg().exp()) + eps_val;
    let h_tilde = (c_matrix / normalizer).matmul(values.clone());

    // --- COMPUTE LAST STATE FOR PREFILL ---
    // Decay to the very end (step S) for each token j: sum_{l=j+1}^{S-1} log f_l
    // log_fg_cumsum[S] - log_fg_cumsum[j+1]
    let total_sum = log_fg_cumsum.clone().narrow(2, s, 1); // (B, NH, 1, 1)
    let cumsum_at_j = log_fg_cumsum.narrow(2, 1, s);     // (B, NH, S, 1)
    let log_decay_to_end = total_sum - cumsum_at_j;     // (B, NH, S, 1)
    
    // Stabilized weights for the state contribution of each token
    let log_w = log_decay_to_end + igate_preact;        // (B, NH, S, 1)
    let m_s = log_w.clone().max_dim(2);                  // (B, NH, 1, 1)
    let w = (log_w - m_s.clone()).exp();                 // (B, NH, S, 1)

    // C_s = sum_j (w_j * k_j^T * v_j) = (K*W)^T @ V
    let keys_scaled = keys * scale;
    let kw = keys_scaled * w.clone();                    // (B, NH, S, DH_qk)
    let next_c = kw.clone().swap_dims(2, 3).matmul(values);     // (B, NH, DH_qk, DH_v)
    
    // n_s = sum_j (w_j * k_j^T) = (K*W)^T @ ones
    let next_n = kw.swap_dims(2, 3).sum_dim(3);         // (B, NH, DH_qk, 1)
    let next_m = m_s;                                   // (B, NH, 1, 1)

    (h_tilde, Some((next_c, next_n, next_m)))
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
    eps_val: f64,
) -> (Tensor<B, 4>, (Tensor<B, 4>, Tensor<B, 4>, Tensor<B, 4>)) {
    let dh = q.dims()[3];
    let scale = 1.0 / (dh as f64).sqrt();

    // log-sigmoid of forget gate: (B, NH, 1, 1)
    let log_fg = burn::tensor::activation::log_sigmoid(fgate_preact);

    // Stabilized max: m_new = max(m + log_fg, igate_preact)
    let m_new = (m_state.clone() + log_fg.clone()).max_pair(igate_preact.clone());

    // Gate activations (stabilized)
    let f_act = (m_state + log_fg - m_new.clone()).exp();   // (B, NH, 1, 1)
    let i_act = (igate_preact - m_new.clone()).exp();        // (B, NH, 1, 1)

    // Scaled key: k / sqrt(DH),  transposed for outer product
    let k_scaled = k * scale;                                // (B, NH, 1, DH)
    let k_t_scaled = k_scaled.swap_dims(2, 3);              // (B, NH, DH, 1)

    // State updates
    // c_new = fg * C + ig * (k^T ⊗ v)    shape: (B, NH, DH_qk, DH_v)
    let c_new = c_state * f_act.clone() + k_t_scaled.clone().matmul(v) * i_act.clone();
    // n_new = fg * n + ig * k^T           shape: (B, NH, DH_qk, 1)
    let n_new = n_state * f_act + k_t_scaled * i_act;

    // Output: h = (q @ C) / max(|q @ n|, exp(-m)) + eps
    let h_num = q.clone().matmul(c_new.clone());             // (B, NH, 1, DH_v)
    let qn_dot = q.matmul(n_new.clone());                    // (B, NH, 1, 1)
    let h_denom = qn_dot.abs().max_pair(m_new.clone().neg().exp()) + eps_val;

    (h_num / h_denom, (c_new, n_new, m_new))
}
