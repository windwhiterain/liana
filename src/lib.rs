use crate::{
    config::{Config, load_config},
    llm::{LLM, llm_from_config},
    state::{State, chat::Chat},
};

pub mod config;
pub mod llm;
pub mod memory;
pub mod state;

pub struct App {
    pub config: Config,
    pub llm: LLM,
    pub state: Box<dyn State>,
}

impl App {
    pub fn new() -> Self {
        let config = load_config();
        let llm = llm_from_config(&config);
        Self {
            config,
            llm,
            state: Box::new(Chat::default()),
        }
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut eframe::egui::Ui, frame: &mut eframe::Frame) {
        ui.horizontal(|ui| {
            self.state.ui(ui, frame);
            if ui.button("run").clicked() {
                self.state.run();
            }
        });
    }
}
