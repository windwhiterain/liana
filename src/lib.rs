use std::sync::Arc;

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
    pub llm: Arc<LLM>,
    pub state: Box<dyn State>,
}

impl App {
    pub fn new() -> Self {
        let config = load_config();
        let llm = Arc::new(llm_from_config(&config));
        Self {
            config,
            llm,
            state: Box::new(Chat::default()),
        }
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut eframe::egui::Ui, frame: &mut eframe::Frame) {
        eframe::egui::Panel::bottom("bottom").show(ui, |ui| {
            self.state.ui(ui, &self.llm);
            if ui.button("Run").clicked() {
                self.state.run(&self.llm);
            }
        });

        eframe::egui::CentralPanel::default().show(ui, |ui| {
            self.state.ui_remainder(ui, frame);
        });
    }
}
