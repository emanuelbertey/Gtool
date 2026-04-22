// sLSTM Cell
// Matches Python: blocks/slstm/cell.py + src/vanilla/slstm.py
//
// The sLSTM extends the classic LSTM with:
// - Exponential gating (stabilized via max trick)
// - An extra normalizer state n (for stable output computation)
// - An extra max state m (for numerical stability)
// - Recurrent connections per head
//
// The cell has 4 states: y (hidden output), c (cell state), n (normalizer), m (max)
// And 4 gates: i (input), f (forget), z (cell input), o (output)

use burn::prelude::*;
use burn::module::{Module, Param};
use burn::config::Config;

#[derive(Config, Debug)]
pub struct SLSTMCellConfig {
    pub hidden_size: usize,
    #[config(default = 4)]
    pub num_heads: usize,
}

/// sLSTM cell state: (y, c, n, m) each of shape (B, H)
pub type SLSTMState<B> = (Tensor<B, 2>, Tensor<B, 2>, Tensor<B, 2>, Tensor<B, 2>);

#[derive(Module, Debug)]
pub struct SLSTMCell<B: Backend> {
    /// Recurrent kernel: (NH, head_dim, 4*head_dim)
    /// Maps previous hidden output y → gate pre-activations Ry
    pub recurrent_kernel: Param<Tensor<B, 3>>,
    /// Bias: (4 * hidden_size,) arranged as [i_bias, f_bias, z_bias, o_bias]
    pub bias: Param<Tensor<B, 1>>,
    pub hidden_size: usize,
    pub num_heads: usize,
    pub head_dim: usize,
}

impl SLSTMCellConfig {
    pub fn init<B: Backend>(&self, device: &B::Device) -> SLSTMCell<B> {
        let head_dim = self.hidden_size / self.num_heads;
        assert!(
            self.hidden_size % self.num_heads == 0,
            "hidden_size must be divisible by num_heads"
        );

        // Recurrent kernel: initialized to zeros (matching Python recurrent_weight_init="zeros")
        let recurrent_kernel = Param::from_tensor(
            Tensor::zeros([self.num_heads, head_dim, 4 * head_dim], device),
        );

        // Bias: zeros, except forget gate gets linspace(3, 6)
        let bias = Param::from_tensor(Tensor::zeros([4 * self.hidden_size], device));

        let mut cell = SLSTMCell {
            recurrent_kernel,
            bias,
            hidden_size: self.hidden_size,
            num_heads: self.num_heads,
            head_dim,
        };
        cell.reset_parameters(device);
        cell
    }
}

impl<B: Backend> SLSTMCell<B> {
    fn reset_parameters(&mut self, device: &B::Device) {
        // Recurrent kernel: zeros
        self.recurrent_kernel = Param::from_tensor(
            Tensor::zeros([self.num_heads, self.head_dim, 4 * self.head_dim], device),
        );

        // Bias: arranged as [i, f, z, o] each of size hidden_size
        // All zeros except f gets linspace(3, 6) per head
        let mut bias_data = vec![0.0f32; 4 * self.hidden_size];
        // f gate starts at offset hidden_size
        for h in 0..self.num_heads {
            for d in 0..self.head_dim {
                let idx = self.hidden_size + h * self.head_dim + d;
                let val = 3.0 + 3.0 * (d as f32) / ((self.head_dim - 1).max(1) as f32);
                bias_data[idx] = val;
            }
        }
        self.bias = Param::from_tensor(
            Tensor::from_floats(bias_data.as_slice(), device),
        );
    }

    /// Recurrent step.
    pub fn step(
        &self,
        x_t: Tensor<B, 2>, // (B, 4*H)
        state: SLSTMState<B>,
    ) -> (Tensor<B, 2>, SLSTMState<B>) {
        let (y, c, n, m) = state;
        let [b, _] = y.dims();
        let bias = self.bias.val().unsqueeze_dim(0); // (1, 4*H)

        // Compute recurrent contribution: Ry = y @ R^T per head
        let y_heads = y.clone().reshape([b, self.num_heads, 1, self.head_dim]);
        let r = self.recurrent_kernel.val().unsqueeze_dim::<4>(0); // (1, NH, DH, 4*DH)
        let ry_heads = y_heads.matmul(r); // (B, NH, 1, 4*DH)
        let ry = ry_heads.reshape([b, 4 * self.hidden_size]);

        let raw = x_t + ry + bias; // (B, 4*H)

        // Split gates
        let iraw = raw.clone().narrow(1, 0, self.hidden_size);
        let fraw = raw.clone().narrow(1, self.hidden_size, self.hidden_size);
        let zraw = raw.clone().narrow(1, 2 * self.hidden_size, self.hidden_size);
        let oraw = raw.narrow(1, 3 * self.hidden_size, self.hidden_size);

        // Pointwise logic
        let logfplusm = m.clone() + log_sigmoid_2d(fraw);
        // Stab: handle init
        let all_n_zero = n.clone().abs().sum().into_scalar().elem::<f32>() < 1e-10;
        let m_new = if all_n_zero {
            iraw.clone()
        } else {
            max_tensor_2d(iraw.clone(), logfplusm.clone())
        };

        let ogate = burn::tensor::activation::sigmoid(oraw);
        let igate = (iraw - m_new.clone()).exp().clamp_max(1.0);
        let fgate = (logfplusm - m_new.clone()).exp().clamp_max(1.0);

        let c_new = fgate.clone() * c + igate.clone() * zraw.tanh();
        let n_new = fgate * n + igate;
        let y_new = ogate * c_new.clone() / n_new.clone();

        (y_new.clone(), (y_new, c_new, n_new, m_new))
    }

    /// Forward: processes a full sequence.
    ///
    /// # Arguments
    /// * `x` — (B, S, 4*H) pre-projected gate inputs [i, f, z, o] concatenated
    /// * `state` — optional initial (y, c, n, m) each (B, H)
    ///
    /// # Returns
    /// * output — (B, S, H)
    /// * final_state — (y, c, n, m)
    pub fn forward(
        &self,
        x: Tensor<B, 3>,
        state: Option<SLSTMState<B>>,
    ) -> (Tensor<B, 3>, SLSTMState<B>) {
        let [b, s, _] = x.dims();
        let device = x.device();

        let mut current_state = match state {
            Some(st) => st,
            None => (
                Tensor::zeros([b, self.hidden_size], &device),
                Tensor::zeros([b, self.hidden_size], &device),
                Tensor::zeros([b, self.hidden_size], &device),
                Tensor::zeros([b, self.hidden_size], &device),
            ),
        };

        let mut outputs = Vec::with_capacity(s);
        for t in 0..s {
            let wx_t = x.clone().narrow(1, t, 1).reshape([b, 4 * self.hidden_size]);
            let (y_t, next_state) = self.step(wx_t, current_state);
            current_state = next_state;
            outputs.push(y_t.unsqueeze_dim(1));
        }

        let output = Tensor::cat(outputs, 1);
        (output, current_state)
    }
}

// ─── Utility functions ─────────────────────────────────────────────────────

fn log_sigmoid_2d<B: Backend>(x: Tensor<B, 2>) -> Tensor<B, 2> {
    let neg_x = x.neg();
    let sp = (neg_x.exp() + 1.0).log();
    sp.neg()
}

fn max_tensor_2d<B: Backend>(a: Tensor<B, 2>, b: Tensor<B, 2>) -> Tensor<B, 2> {
    let mask = a.clone().greater_equal(b.clone());
    a.mask_where(mask.bool_not(), b)
}
