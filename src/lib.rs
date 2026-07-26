use std::sync::Arc;

use eframe::egui::{self, Align, Layout};

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
    pub state: Option<Box<dyn State>>,
    pub memory_manager: memory::Manager,
}

impl App {
    pub fn new() -> Self {
        let config = load_config();
        let llm = Arc::new(llm_from_config(&config));
        Self {
            config,
            llm,
            state: Some(Box::new(Chat::default())),
            memory_manager: Default::default(),
        }
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut eframe::egui::Ui, _frame: &mut eframe::Frame) {
        let mut state = std::mem::take(&mut self.state).unwrap();
        state.poll(self);
        eframe::egui::Panel::bottom("bottom").show(ui, |ui| {
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui
                    .add_sized(
                        egui::vec2(0.0, ui.available_height()),
                        egui::Button::new("run"),
                    )
                    .clicked()
                {
                    state.run(self);
                }
                state.ui(ui);
            });
        });

        eframe::egui::CentralPanel::default().show(ui, |ui| {
            state.ui_remainder(ui);
        });
        if self.state.is_none() {
            self.state = Some(state)
        }
    }
}
