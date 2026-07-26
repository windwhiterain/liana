use rig::{client::Client, providers::openai};

use crate::config::Config;

pub type LLM = Client<openai::OpenAIResponsesExt>;

pub fn llm_from_config(config: &Config) -> LLM {
    openai::Client::builder()
        .api_key(&config.api_key)
        .base_url(&config.base_url)
        .build()
        .expect("Failed to build LLM client")
}
