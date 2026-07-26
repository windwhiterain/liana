pub mod chat;

pub trait State {
    fn ui(&mut self, ui: &mut eframe::egui::Ui, frame: &mut eframe::Frame);
    fn run(&mut self);
}
