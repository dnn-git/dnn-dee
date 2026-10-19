//! Bộ phát nhạc: bọc phần tử `playbin` của GStreamer.

use std::path::Path;

use gstreamer as gst;
use gst::prelude::*;

pub use gst::bus::BusWatchGuard;

pub struct Player {
    playbin: gst::Element,
}

impl Player {
    pub fn new() -> Result<Self, gst::glib::BoolError> {
        let playbin = gst::ElementFactory::make("playbin")
            .name("dee-player")
            .build()?;

        // Chỉ phát âm thanh: nếu tệp có luồng video/ảnh bìa thì bỏ qua,
        // tránh playbin tự mở cửa sổ video.
        if let Ok(sink) = gst::ElementFactory::make("fakesink").build() {
            playbin.set_property_from_value("video-sink", &sink.to_value());
        }

        Ok(Self { playbin })
    }

    /// Gắn bộ theo dõi bus. Giữ giá trị trả về để watch không bị huỷ.
    pub fn watch(
        &self,
        on_eos: impl Fn() + 'static,
        on_error: impl Fn(String) + 'static,
    ) -> Result<BusWatchGuard, gst::glib::BoolError> {
        let bus = self.playbin.bus().expect("playbin luôn có bus");
        bus.add_watch_local(move |_, message| {
            use gst::MessageView;
            match message.view() {
                MessageView::Eos(_) => on_eos(),
                MessageView::Error(err) => {
                    let detail = err.debug().map(|d| d.to_string()).unwrap_or_default();
                    on_error(format!("{} ({})", err.error(), detail));
                }
                _ => {}
            }
            gst::glib::ControlFlow::Continue
        })
    }

    pub fn play_file(&self, path: &Path) {
        let Ok(uri) = gst::glib::filename_to_uri(path, None) else {
            eprintln!("Không tạo được URI cho {}", path.display());
            return;
        };
        let _ = self.playbin.set_state(gst::State::Null);
        self.playbin.set_property("uri", uri.as_str());
        let _ = self.playbin.set_state(gst::State::Playing);
    }

    pub fn set_playing(&self, playing: bool) {
        let state = if playing {
            gst::State::Playing
        } else {
            gst::State::Paused
        };
        let _ = self.playbin.set_state(state);
    }

    pub fn stop(&self) {
        let _ = self.playbin.set_state(gst::State::Null);
    }

    pub fn is_playing(&self) -> bool {
        self.playbin.current_state() == gst::State::Playing
    }

    /// Vị trí hiện tại, tính bằng giây.
    pub fn position(&self) -> Option<f64> {
        self.playbin
            .query_position::<gst::ClockTime>()
            .map(|t| t.seconds_f64())
    }

    /// Tổng thời lượng, tính bằng giây.
    pub fn duration(&self) -> Option<f64> {
        self.playbin
            .query_duration::<gst::ClockTime>()
            .map(|t| t.seconds_f64())
    }

    pub fn seek(&self, seconds: f64) {
        let nanos = (seconds.max(0.0) * 1e9) as u64;
        let _ = self.playbin.seek_simple(
            gst::SeekFlags::FLUSH | gst::SeekFlags::KEY_UNIT,
            gst::ClockTime::from_nseconds(nanos),
        );
    }

    /// `volume`: 0.0 ..= 1.0
    pub fn set_volume(&self, volume: f64) {
        self.playbin.set_property("volume", volume);
    }
}

impl Drop for Player {
    fn drop(&mut self) {
        let _ = self.playbin.set_state(gst::State::Null);
    }
}
