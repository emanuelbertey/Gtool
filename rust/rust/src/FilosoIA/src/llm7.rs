use std::io::{self, Write};
use edgequake_llm::{
    LLMProvider, OpenAICompatibleProvider,
    model_config::{ProviderConfig, ProviderType},
};
use futures::StreamExt;

pub struct Llm7Chat {
    provider: OpenAICompatibleProvider,
}

impl Llm7Chat {
    pub fn new(api_key: &str, model: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let config = ProviderConfig {
            name: "llm7".to_string(),
            display_name: "LLM7.io".to_string(),
            provider_type: ProviderType::OpenAICompatible,
            base_url: Some("https://api.llm7.io/v1".to_string()),
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


