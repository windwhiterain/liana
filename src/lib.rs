use crate::{
    config::{Config, load_config},
    llm::{LLM, llm_from_config},
};

pub mod config;
pub mod llm;

pub struct App {
    pub config: Config,
    pub llm: LLM,
}

impl App {
    pub fn new() -> Self {
        let config = load_config();
        let llm = llm_from_config(&config);
        Self { config, llm }
    }
}
