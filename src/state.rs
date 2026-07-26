use std::sync::Arc;

use crate::llm::LLM;

pub mod chat;

pub trait State {
    fn ui_remainder(&mut self, ui: &mut eframe::egui::Ui, frame: &mut eframe::Frame);
    fn ui(&mut self, ui: &mut eframe::egui::Ui, llm: &Arc<LLM>);
    fn run(&mut self, llm: &Arc<LLM>);
}
