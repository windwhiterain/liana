use crate::App;

pub mod chat;

pub trait State {
    fn ui_remainder(&mut self, ui: &mut eframe::egui::Ui);
    fn ui(&mut self, ui: &mut eframe::egui::Ui);
    fn run(&mut self, app: &mut App);
    fn poll(&mut self, app: &mut App);
}
