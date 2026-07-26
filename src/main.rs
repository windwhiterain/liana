use eframe::egui;
use liana::App;

#[tokio::main]
async fn main() {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([320.0, 240.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Liana",
        options,
        Box::new(|_| {Ok(Box::new(App::new()))}),
    ).unwrap();
}
