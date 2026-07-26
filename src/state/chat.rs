use egui_probe::EguiProbe;
use rig::message::Message;

use crate::state::State;

#[derive(Default)]
pub struct Chat {
    pub messages: Vec<Message>,
    pub ui: UI,
}

#[derive(Default, EguiProbe)]
pub struct UI {
    pub message: String,
}

impl State for Chat {
    fn ui(&mut self, ui: &mut eframe::egui::Ui, frame: &mut eframe::Frame) {
        self.ui.probe(ui, &Default::default());
    }

    fn run(&mut self) {}
}
