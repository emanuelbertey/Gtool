use std::io::{self, Write};
use edgequake_llm::{
    LLMProvider, OpenAICompatibleProvider,
    model_config::{ProviderConfig, ProviderType},
};
use futures::StreamExt;

pub struct GrokChat {
    provider: OpenAICompatibleProvider,
}

impl GrokChat {
    pub fn new(api_key: &str, model: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let config = ProviderConfig {
            name: "xai".to_string(),
            display_name: "xAI Grok".to_string(),
            provider_type: ProviderType::OpenAICompatible,
            base_url: Some("https://api.x.ai/v1".to_string()),
            api_key: Some(api_key.to_string()),
            default_llm_model: Some(model.to_string()),
            ..Default::default()
        };
        let provider = OpenAICompatibleProvider::from_config(config)?
            .with_model(model);
        Ok(Self { provider })
    }

    pub async fn chat_stream(&self, prompt: &str) -> Result<(), Box<dyn std::error::Error>> {
        let mut stream = self.provider.stream(prompt).await?;
        while let Some(result) = stream.next().await {
            let content = result?;
            print!("{content}");
            io::stdout().flush()?;
        }
        println!();
        Ok(())
    }

    pub fn as_provider(&self) -> &dyn LLMProvider {
        &self.provider
    }
}


