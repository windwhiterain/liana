use iced::window;

#[tokio::main]
async fn main() -> iced::Result {
    iced::application(liana::init, liana::update, liana::view)
        .window(window::Settings {
            size: iced::Size::new(800.0, 600.0),
            ..Default::default()
        })
        .theme(liana::theme)
        .antialiasing(true)
        .run()
}
