use std::io::{self, Write};
use edgequake_llm::{
    LLMProvider, OpenAICompatibleProvider,
    model_config::{ProviderConfig, ProviderType},
};
use futures::StreamExt;

pub struct CloudflareChat {
    provider: OpenAICompatibleProvider,
}

impl CloudflareChat {
    pub fn new(api_token: &str, account_id: &str, model: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let base_url = format!("https://api.cloudflare.com/client/v4/accounts/{}/ai/v1", account_id);
        let config = ProviderConfig {
            name: "cloudflare".to_string(),
            display_name: "Cloudflare AI Gateway".to_string(),
            provider_type: ProviderType::OpenAICompatible,
            base_url: Some(base_url),
            api_key: Some(api_token.to_string()),
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

pub fn prompt_api_token() -> (String, String) {
    print!("Enter your Cloudflare API token: ");
    io::stdout().flush().unwrap();
    let mut token = String::new();
    io::stdin().read_line(&mut token).unwrap();

    print!("Enter your Cloudflare Account ID: ");
    io::stdout().flush().unwrap();
    let mut id = String::new();
    io::stdin().read_line(&mut id).unwrap();

    (token.trim().to_string(), id.trim().to_string())
}
