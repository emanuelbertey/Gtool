use burn::prelude::*;
use burn::module::Module;

pub fn soft_cap<B: Backend, const D: usize>(values: Tensor<B, D>, cap_value: Option<f64>) -> Tensor<B, D> {
    match cap_value {
        Some(cap) => {
            values.div_scalar(cap).tanh().mul_scalar(cap)
        }
        None => values,
    }
}

#[derive(Module, Debug)]
pub struct RMSNorm<B: Backend> {
    pub weight: Option<burn::module::Param<Tensor<B, 1>>>,
    pub bias: Option<burn::module::Param<Tensor<B, 1>>>,
    pub eps: f64,
    pub force_float32_reductions: bool,
}

impl<B: Backend> RMSNorm<B> {
    pub fn init(
        dim: usize, 
        use_weight: bool, 
        use_bias: bool, 
        eps: f64, 
        force_float32_reductions: bool,
        device: &B::Device
    ) -> Self {
        let weight = if use_weight {
            Some(burn::module::Param::from_tensor(Tensor::ones([dim], device)))
        } else {
            None
        };
        let bias = if use_bias {
            Some(burn::module::Param::from_tensor(Tensor::zeros([dim], device)))
        } else {
            None
        };
        Self { weight, bias, eps, force_float32_reductions }
    }

    fn rms_normalize<const D: usize>(&self, x: Tensor<B, D>) -> Tensor<B, D> {
        // x: (B, ..., D)
        let var = x.clone().powf_scalar(2.0).mean_dim(D - 1);
        let inv_std = var.add_scalar(self.eps).sqrt().recip();
        x.mul(inv_std)
    }

    fn apply_weight_bias<const D: usize>(&self, x: Tensor<B, D>) -> Tensor<B, D> {
        let mut out = x;
        if let Some(w) = &self.weight {
            let w_val = w.val();
            let mut shape = [1; D];
            shape[D - 1] = w_val.dims()[0];
            out = out.mul(w_val.reshape(shape));
        }
        if let Some(b) = &self.bias {
            let b_val = b.val();
            let mut shape = [1; D];
            shape[D - 1] = b_val.dims()[0];
            out = out.add(b_val.reshape(shape));
        }
        out
    }

    pub fn forward<const D: usize>(&self, x: Tensor<B, D>) -> Tensor<B, D> {
        let x = self.rms_normalize(x);
        self.apply_weight_bias(x)
    }
}

#[derive(Module, Debug)]
pub struct MultiHeadRMSNorm<B: Backend> {
    pub norm: RMSNorm<B>,
    pub num_heads: usize,
    pub head_dim: usize,
}

impl<B: Backend> MultiHeadRMSNorm<B> {
    pub fn init(
        num_heads: usize, 
        head_dim: usize, 
        use_weight: bool, 
        use_bias: bool, 
        eps: f64, 
        force_float32_reductions: bool,
        device: &B::Device
    ) -> Self {
        Self {
            norm: RMSNorm::init(num_heads * head_dim, use_weight, use_bias, eps, force_float32_reductions, device),
            num_heads,
            head_dim,
        }
    }

    pub fn forward(&self, x: Tensor<B, 4>) -> Tensor<B, 3> {
        let [b, s, nh, dh] = x.dims();
        let x = self.norm.rms_normalize(x);
        let x_reshaped = x.reshape([b, s, nh * dh]);
        self.norm.apply_weight_bias(x_reshaped)
    }
}

#[derive(Module, Debug)]
pub struct LayerNorm<B: Backend> {
    pub weight: Option<burn::module::Param<Tensor<B, 1>>>,
    pub bias: Option<burn::module::Param<Tensor<B, 1>>>,
    pub eps: f64,
    pub force_float32_reductions: bool,
}

impl<B: Backend> LayerNorm<B> {
    pub fn init(
        dim: usize, 
        use_weight: bool, 
        use_bias: bool, 
        eps: f64, 
        force_float32_reductions: bool,
        device: &B::Device
    ) -> Self {
        let weight = if use_weight {
            Some(burn::module::Param::from_tensor(Tensor::ones([dim], device)))
        } else {
            None
        };
        let bias = if use_bias {
            Some(burn::module::Param::from_tensor(Tensor::zeros([dim], device)))
        } else {
            None
        };
        Self { weight, bias, eps, force_float32_reductions }
    }

    fn layer_normalize<const D: usize>(&self, x: Tensor<B, D>) -> Tensor<B, D> {
        // Matches Python: x_centered = x - mean(x), var = mean(x^2) - mean(x)^2
        let mean = x.clone().mean_dim(D - 1);
        let x_centered = x.clone().sub(mean.clone());
        // var = E[x^2] - E[x]^2  (equivalent to torch.var with unbiased=False)
        let var = x.powf_scalar(2.0).mean_dim(D - 1).sub(mean.powf_scalar(2.0));
        let inv_std = var.add_scalar(self.eps).sqrt().recip();
        x_centered.mul(inv_std)
    }

    fn apply_weight_bias<const D: usize>(&self, x: Tensor<B, D>) -> Tensor<B, D> {
        let mut out = x;
        if let Some(w) = &self.weight {
            let w_val = w.val();
            let mut shape = [1; D];
            shape[D - 1] = w_val.dims()[0];
            out = out.mul(w_val.reshape(shape));
        }
        if let Some(b) = &self.bias {
            let b_val = b.val();
            let mut shape = [1; D];
            shape[D - 1] = b_val.dims()[0];
            out = out.add(b_val.reshape(shape));
        }
        out
    }

    pub fn forward<const D: usize>(&self, x: Tensor<B, D>) -> Tensor<B, D> {
        let x = self.layer_normalize(x);
        self.apply_weight_bias(x)
    }
}

#[derive(Module, Debug)]
pub struct MultiHeadLayerNorm<B: Backend> {
    pub norm: LayerNorm<B>,
    pub num_heads: usize,
    pub head_dim: usize,
}

impl<B: Backend> MultiHeadLayerNorm<B> {
    pub fn init(
        num_heads: usize, 
        head_dim: usize, 
        use_weight: bool, 
        use_bias: bool, 
        eps: f64, 
        force_float32_reductions: bool,
        device: &B::Device
    ) -> Self {
        Self {
            norm: LayerNorm::init(num_heads * head_dim, use_weight, use_bias, eps, force_float32_reductions, device),
            num_heads,
            head_dim,
        }
    }

    pub fn forward(&self, x: Tensor<B, 4>) -> Tensor<B, 3> {
        // Python normalizes over DH per head (last dim of (B,S,NH,DH)),
        // then reshapes to (B,S,NH*DH) and applies weight/bias.
        let [b, s, nh, dh] = x.dims();
        let x_norm = self.norm.layer_normalize(x); // normalizes over last dim (DH)
        let x_reshaped = x_norm.reshape([b, s, nh * dh]);
        self.norm.apply_weight_bias(x_reshaped)
    }
}
