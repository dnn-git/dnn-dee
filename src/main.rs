//! Dee Offline Music - trình phát nhạc ngoại tuyến (GTK4 + GStreamer).

mod app;
mod config;
mod library;
mod player;
mod theme;

use gtk::prelude::*;

fn main() -> gtk::glib::ExitCode {
    if let Err(error) = gstreamer::init() {
        eprintln!("Không khởi tạo được GStreamer: {error}");
        return gtk::glib::ExitCode::FAILURE;
    }

    let application = gtk::Application::builder()
        .application_id("vn.dnn.Dee")
        .build();
    application.connect_activate(app::build);
    application.run()
}
