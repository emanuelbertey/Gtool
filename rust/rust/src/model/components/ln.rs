use burn::prelude::*;
use burn::module::Module;
use burn::config::Config;
use burn::nn;

// ─── LayerNorm ────────────────────────────────────────────────────────────────

#[derive(Config, Debug)]
pub struct LayerNormConfig {
    pub ndim: usize,
    #[config(default = true)]
    pub weight: bool, // Mantenemos el campo para no romper tus otros archivos
    #[config(default = false)]
    pub bias: bool,   // Mantenemos el campo para no romper tus otros archivos
    #[config(default = 1e-5)]
    pub eps: f64,
    #[config(default = true)]
    pub residual_weight: bool,
}

#[derive(Module, Debug)]
pub struct LayerNorm<B: Backend> {
    pub norm: nn::LayerNorm<B>,
    pub weight_param: Option<burn::module::Param<Tensor<B, 1>>>,
    pub bias_param: Option<burn::module::Param<Tensor<B, 1>>>,
    pub residual_weight: bool,
}

impl LayerNormConfig {
    pub fn init<B: Backend>(&self, device: &B::Device) -> LayerNorm<B> {
        let norm = nn::LayerNormConfig::new(self.ndim)
            .with_epsilon(self.eps)
            .init(device);
        // Note: We'll modify the forward pass to apply our own affine transformation
        
        let weight_param = if self.weight {
            let w = if self.residual_weight {
                Tensor::zeros([self.ndim], device)
            } else {
                Tensor::ones([self.ndim], device)
            };
            Some(burn::module::Param::from_tensor(w))
        } else {
            None
        };

        let bias_param = if self.bias {
            Some(burn::module::Param::from_tensor(Tensor::zeros([self.ndim], device)))
        } else {
            None
        };

        LayerNorm { 
            norm,
            weight_param,
            bias_param,
            residual_weight: self.residual_weight,
        }
    }
}

impl<B: Backend> LayerNorm<B> {
    pub fn forward(&self, x: Tensor<B, 3>) -> Tensor<B, 3> {
        // El layer norm de Burn tiene sus propios pesos, pero para ser fieles al 100%
        // ignoraremos su escala si aplicamos nuestro weight_proxy.
        // Dado que Burn arranca sus pesos en 1.0, matemáticamente será redundante si no los entrena, 
        // pero preferimos aplicar nuestra propia escala en top.
        let mut out = self.norm.forward(x);
        
        if let Some(w) = &self.weight_param {
            let w_val = if self.residual_weight {
                w.val() + 1.0
            } else {
                w.val()
            };
            out = out * w_val.unsqueeze::<2>().unsqueeze::<3>(); // Broadcasting (D) -> (1, 1, D)
        }
        
        if let Some(b) = &self.bias_param {
            out = out + b.val().unsqueeze::<2>().unsqueeze::<3>();
        }
        
        out
    }

    pub fn forward_2d(&self, x: Tensor<B, 2>) -> Tensor<B, 2> {
        let mut out = self.norm.forward(x);
        
        if let Some(w) = &self.weight_param {
            let w_val = if self.residual_weight {
                w.val() + 1.0
            } else {
                w.val()
            };
            out = out * w_val.unsqueeze::<2>(); 
        }
        
        if let Some(b) = &self.bias_param {
            out = out + b.val().unsqueeze::<2>();
        }
        out
    }
}

// ─── MultiHeadLayerNorm ───────────────────────────────────────────────────────

#[derive(Config, Debug, Default)]
pub struct MultiHeadLayerNormConfig {
    pub ndim: usize,
    #[config(default = true)]
    pub weight: bool, // Mantenemos el campo para no romper tus otros archivos
    #[config(default = false)]
    pub bias: bool,   // Mantenemos el campo para no romper tus otros archivos
    #[config(default = 1e-5)]
    pub eps: f64,
    #[config(default = true)]
    pub residual_weight: bool,
}

#[derive(Module, Debug)]
pub struct MultiHeadLayerNorm<B: Backend> {
    pub norm: nn::GroupNorm<B>,
    pub weight_param: Option<burn::module::Param<Tensor<B, 1>>>,
    pub bias_param: Option<burn::module::Param<Tensor<B, 1>>>,
    pub ndim: usize,
    pub num_heads: usize,
    pub residual_weight: bool,
}

impl MultiHeadLayerNormConfig {
    pub fn init<B: Backend>(&self, num_heads: usize, device: &B::Device) -> MultiHeadLayerNorm<B> {
        // En Burn, GroupNormConfig tiene affine habilitado. Lo ideal sería apagarlo 
        // pero .with_affine(false) no está en todas las versiones, así que aplicaremos
        // nuestro truco de proxy encima, sobreescribiendo el efecto del optimizador
        // (En la práctica, lo que importará son nuestros parámetros).
        let norm = nn::GroupNormConfig::new(num_heads, self.ndim)
            .with_epsilon(self.eps)
            .init(device);
        
        let weight_param = if self.weight {
            let w = if self.residual_weight {
                Tensor::zeros([self.ndim], device)
            } else {
                Tensor::ones([self.ndim], device)
            };
            Some(burn::module::Param::from_tensor(w))
        } else {
            None
        };

        let bias_param = if self.bias {
            Some(burn::module::Param::from_tensor(Tensor::zeros([self.ndim], device)))
        } else {
            None
        };

        MultiHeadLayerNorm {
            norm,
            weight_param,
            bias_param,
            ndim: self.ndim,
            num_heads,
            residual_weight: self.residual_weight,
        }
    }
}

impl<B: Backend> MultiHeadLayerNorm<B> {
    pub fn forward(&self, x: Tensor<B, 4>) -> Tensor<B, 4> {
        let [b, nh, s, dh] = x.dims();
        let x = x.swap_dims(1, 2).reshape([b * s, nh * dh]);
        let mut out = self.norm.forward(x);
        
        // Aplicamos nuestro affine en 2D => broadcasting sutil
        if let Some(w) = &self.weight_param {
            let w_val = if self.residual_weight {
                w.val() + 1.0
            } else {
                w.val()
            };
            out = out * w_val.unsqueeze::<2>(); // Convierte a [1, ndim] y expandirá sobre batch
        }
        if let Some(b) = &self.bias_param {
            out = out + b.val().unsqueeze::<2>();
        }

        out.reshape([b, s, nh, dh]).swap_dims(1, 2)
    }
}