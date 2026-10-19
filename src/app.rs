//! Giao diện và logic chính của Dee.

use std::cell::{Cell, RefCell};
use std::f64::consts::TAU;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::Duration;

use gtk::glib::clone;
use gtk::prelude::*;
use gtk::{cairo, gdk, gio, glib, pango};

use crate::config;
use crate::library;
use crate::player::{BusWatchGuard, Player};
use crate::theme::{Rgb, Theme};

const BASE_CSS: &str = include_str!("style.css");

const ICON_PLAY: &str = "media-playback-start-symbolic";
const ICON_PAUSE: &str = "media-playback-pause-symbolic";

/// Thứ tự: lặp tất cả, lặp một bài, ngẫu nhiên.
const MODE_TIPS: [&str; 3] = ["Lặp lại tất cả", "Lặp lại một bài", "Phát ngẫu nhiên"];

// ---------------------------------------------------------------------------
// Widget
// ---------------------------------------------------------------------------

struct Widgets {
    window: gtk::ApplicationWindow,
    search: gtk::SearchEntry,
    menu_popover: gtk::Popover,
    menu_status: gtk::Label,
    menu_items: Vec<gtk::Button>,
    list: gtk::ListBox,
    row_popover: gtk::Popover,
    remove_button: gtk::Button,
    repeat_all: gtk::ToggleButton,
    repeat_one: gtk::ToggleButton,
    shuffle: gtk::ToggleButton,
    disc: gtk::DrawingArea,
    title: gtk::Label,
    counter: gtk::Label,
    artist: gtk::Label,
    progress: gtk::Scale,
    elapsed: gtk::Label,
    total: gtk::Label,
    prev: gtk::Button,
    play: gtk::Button,
    next: gtk::Button,
    volume: gtk::Scale,
}

#[derive(Default)]
struct State {
    /// Toàn bộ bài trong thư mục đang mở.
    all: Vec<PathBuf>,
    /// Danh sách đang hiển thị (sau khi lọc theo ô tìm kiếm).
    files: Vec<PathBuf>,
    /// Bài đang phát (nhớ theo đường dẫn nên không lệch khi lọc/xóa bài).
    now_playing: Option<PathBuf>,
}

pub struct Dee {
    w: Widgets,
    player: Player,
    st: RefCell<State>,
    theme: Cell<Theme>,
    angle: Cell<f64>,
    duration: Cell<f64>,
    syncing: Cell<bool>,
    pending_remove: RefCell<Option<PathBuf>>,
    css: gtk::CssProvider,
    bus_guard: RefCell<Option<BusWatchGuard>>,
}

// ---------------------------------------------------------------------------
// Khởi tạo
// ---------------------------------------------------------------------------

pub fn build(app: &gtk::Application) {
    if let Some(window) = app.active_window() {
        window.present();
        return;
    }

    let player = match Player::new() {
        Ok(player) => player,
        Err(error) => {
            eprintln!(
                "Không tạo được GStreamer playbin: {error}\n\
                 Hãy cài gstreamer1.0-plugins-base và gstreamer1.0-plugins-good."
            );
            app.quit();
            return;
        }
    };

    gtk::Window::set_default_icon_name("vn.dnn.Dee");

    let css = gtk::CssProvider::new();
    if let Some(display) = gdk::Display::default() {
        gtk::style_context_add_provider_for_display(
            &display,
            &css,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }

    let widgets = build_widgets(app);
    let saved_theme = Theme::from_name(&config::get_string("theme").unwrap_or_default());

    let dee = Rc::new(Dee {
        w: widgets,
        player,
        st: RefCell::new(State::default()),
        theme: Cell::new(saved_theme),
        angle: Cell::new(0.0),
        duration: Cell::new(0.0),
        syncing: Cell::new(false),
        pending_remove: RefCell::new(None),
        css,
        bus_guard: RefCell::new(None),
    });

    dee.apply_theme(saved_theme, false);
    dee.wire();
    dee.restore_music_folder();
    dee.w.window.present();
}

fn icon_button(icon: &str) -> gtk::Button {
    let button = gtk::Button::from_icon_name(icon);
    button.add_css_class("music-control");
    button
}

fn update_mode_button(button: &gtk::ToggleButton, tip: &str) {
    if button.is_active() {
        button.add_css_class("mode-active");
        button.set_tooltip_text(Some(&format!("{tip} — ĐANG BẬT")));
    } else {
        button.remove_css_class("mode-active");
        button.set_tooltip_text(Some(tip));
    }
}

fn build_widgets(app: &gtk::Application) -> Widgets {
    let window = gtk::ApplicationWindow::builder()
        .application(app)
        .title("Dee Offline Music")
        .decorated(false)
        .resizable(false)
        .default_width(650)
        .default_height(380)
        .build();
    window.set_size_request(650, 380);

    let shell = gtk::Box::new(gtk::Orientation::Vertical, 0);
    window.set_child(Some(&shell));

    // ---- Thanh tiêu đề riêng ------------------------------------------------
    let logo = gtk::DrawingArea::builder()
        .content_width(28)
        .content_height(28)
        .build();
    logo.set_draw_func(|_, cr, w, h| draw_small_vinyl(cr, f64::from(w), f64::from(h)));

    let brand_name = gtk::Label::new(Some("Dee"));
    brand_name.add_css_class("brand-name");
    let brand_subtitle = gtk::Label::new(Some("Offline Music"));
    brand_subtitle.add_css_class("brand-subtitle");

    let brand = gtk::Box::new(gtk::Orientation::Horizontal, 5);
    brand.add_css_class("brand");
    brand.append(&brand_name);
    brand.append(&brand_subtitle);

    let brand_group = gtk::Box::new(gtk::Orientation::Horizontal, 9);
    brand_group.set_halign(gtk::Align::Center);
    brand_group.set_valign(gtk::Align::Center);
    brand_group.append(&logo);
    brand_group.append(&brand);

    let minimize_button = gtk::Button::from_icon_name("window-minimize-symbolic");
    minimize_button.set_tooltip_text(Some("Thu nhỏ"));
    minimize_button.add_css_class("window-button");
    minimize_button.connect_clicked(clone!(
        #[weak]
        window,
        move |_| window.minimize()
    ));

    let close_button = gtk::Button::from_icon_name("window-close-symbolic");
    close_button.set_tooltip_text(Some("Đóng Dee"));
    close_button.add_css_class("window-button");
    close_button.add_css_class("close-button");
    close_button.connect_clicked(clone!(
        #[weak]
        window,
        move |_| window.close()
    ));

    let window_buttons = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    window_buttons.set_valign(gtk::Align::Center);
    window_buttons.set_margin_end(8);
    window_buttons.append(&minimize_button);
    window_buttons.append(&close_button);

    // CenterBox giữ tên ứng dụng ở chính giữa, độc lập với hai nút bên phải.
    let bar = gtk::CenterBox::new();
    bar.add_css_class("titlebar");
    bar.set_size_request(-1, 32);
    bar.set_center_widget(Some(&brand_group));
    bar.set_end_widget(Some(&window_buttons));

    // WindowHandle cho phép kéo thanh tiêu đề để di chuyển cửa sổ.
    let handle = gtk::WindowHandle::new();
    handle.set_child(Some(&bar));
    shell.append(&handle);

    // ---- Nội dung chính -------------------------------------------------------
    let main = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    main.set_vexpand(true);
    shell.append(&main);

    // ---- Bảng trái: tìm kiếm + danh sách + chế độ phát ------------------------
    let left = gtk::Box::new(gtk::Orientation::Vertical, 14);
    left.set_size_request(275, -1);
    left.set_margin_top(22);
    left.set_margin_bottom(22);
    left.set_margin_start(20);
    left.set_margin_end(18);
    left.add_css_class("left-panel");
    main.append(&left);

    let top = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    left.append(&top);

    let search = gtk::SearchEntry::new();
    search.set_placeholder_text(Some("Tìm bài hát..."));
    search.set_hexpand(true);
    top.append(&search);

    // Menu ba gạch
    let menu_box = gtk::Box::new(gtk::Orientation::Vertical, 2);
    menu_box.set_margin_top(8);
    menu_box.set_margin_bottom(8);
    menu_box.set_margin_start(8);
    menu_box.set_margin_end(8);

    let menu_specs = [
        ("Mở thư mục nhạc", "folder-open-symbolic"),
        ("Giao diện", "applications-graphics-symbolic"),
        ("Cài đặt", "preferences-system-symbolic"),
        ("Shortcut", "input-keyboard-symbolic"),
        ("Giới thiệu Dee", "help-about-symbolic"),
    ];
    let mut menu_items = Vec::new();
    for (label, icon_name) in menu_specs {
        let item = gtk::Button::new();
        item.add_css_class("flat");
        item.set_halign(gtk::Align::Fill);

        let row = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        row.set_margin_top(6);
        row.set_margin_bottom(6);
        row.set_margin_start(6);
        row.set_margin_end(6);

        let icon = gtk::Image::from_icon_name(icon_name);
        icon.set_pixel_size(18);
        row.append(&icon);

        let text = gtk::Label::new(Some(label));
        text.set_xalign(0.0);
        text.set_hexpand(true);
        row.append(&text);

        item.set_child(Some(&row));
        menu_box.append(&item);
        menu_items.push(item);
    }

    let menu_status = gtk::Label::new(None);
    menu_status.set_wrap(true);
    menu_status.set_margin_top(4);
    menu_status.set_margin_start(8);
    menu_status.set_margin_end(8);
    menu_box.append(&menu_status);

    let menu_popover = gtk::Popover::builder()
        .position(gtk::PositionType::Bottom)
        .child(&menu_box)
        .build();
    menu_popover.add_css_class("menu-popover");

    let menu_button = gtk::MenuButton::builder()
        .icon_name("open-menu-symbolic")
        .tooltip_text("Menu")
        .popover(&menu_popover)
        .build();
    top.append(&menu_button);

    let heading = gtk::Label::new(Some("Thư viện nhạc"));
    heading.set_xalign(0.0);
    heading.add_css_class("muted");
    left.append(&heading);

    let list = gtk::ListBox::new();
    list.set_selection_mode(gtk::SelectionMode::Single);
    list.set_hexpand(true);

    let list_scroll = gtk::ScrolledWindow::new();
    list_scroll.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
    list_scroll.set_hexpand(true);
    list_scroll.set_vexpand(true);
    list_scroll.set_child(Some(&list));
    left.append(&list_scroll);

    // Menu chuột phải cho từng bài: một popover dùng chung, gắn vào danh sách.
    let remove_button = gtk::Button::with_label("Xóa");
    remove_button.add_css_class("flat");
    remove_button.set_halign(gtk::Align::Fill);

    let row_menu = gtk::Box::new(gtk::Orientation::Vertical, 0);
    row_menu.append(&remove_button);

    let row_popover = gtk::Popover::new();
    row_popover.add_css_class("menu-popover");
    row_popover.set_child(Some(&row_menu));
    row_popover.set_has_arrow(true);
    row_popover.set_parent(&list);

    // Ba nút chế độ phát (loại trừ lẫn nhau)
    let repeat_all = gtk::ToggleButton::new();
    repeat_all.set_icon_name("media-playlist-repeat-symbolic");

    let repeat_one = gtk::ToggleButton::new();
    let repeat_icon = gtk::Image::from_icon_name("media-playlist-repeat-symbolic");
    repeat_icon.set_pixel_size(20);
    let one_badge = gtk::Label::new(Some("1"));
    one_badge.set_halign(gtk::Align::End);
    one_badge.set_valign(gtk::Align::End);
    one_badge.add_css_class("repeat-one-badge");
    let icon_box = gtk::Overlay::new();
    icon_box.set_size_request(24, 24);
    icon_box.set_child(Some(&repeat_icon));
    icon_box.add_overlay(&one_badge);
    repeat_one.set_child(Some(&icon_box));

    let shuffle = gtk::ToggleButton::new();
    shuffle.set_icon_name("media-playlist-shuffle-symbolic");

    let mode_row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    mode_row.set_halign(gtk::Align::Center);
    for (button, tip) in [&repeat_all, &repeat_one, &shuffle].into_iter().zip(MODE_TIPS) {
        button.set_size_request(40, 38);
        button.add_css_class("music-control");
        update_mode_button(button, tip);
        mode_row.append(button);
    }
    left.append(&mode_row);

    // ---- Bảng phải: đĩa than + thông tin + điều khiển -------------------------
    let right = gtk::Box::new(gtk::Orientation::Vertical, 12);
    right.set_hexpand(true);
    right.set_vexpand(true);
    right.set_margin_top(24);
    right.set_margin_bottom(24);
    right.set_margin_start(20);
    right.set_margin_end(20);
    main.append(&right);

    let disc = gtk::DrawingArea::builder()
        .content_width(320)
        .content_height(180)
        .hexpand(true)
        .vexpand(true)
        .build();
    right.append(&disc);

    let title = gtk::Label::new(Some("Chọn một bài hát"));
    title.set_ellipsize(pango::EllipsizeMode::End);
    title.set_max_width_chars(28);
    title.set_single_line_mode(true);
    title.add_css_class("title");
    right.append(&title);

    let counter = gtk::Label::new(Some("—/0"));
    counter.add_css_class("artist");
    right.append(&counter);

    let artist = gtk::Label::new(Some("Dee · Offline Music"));
    artist.add_css_class("artist");
    right.append(&artist);

    let progress = gtk::Scale::with_range(gtk::Orientation::Horizontal, 0.0, 100.0, 1.0);
    progress.set_draw_value(false);
    progress.add_css_class("music-scale");
    progress.set_halign(gtk::Align::Center);
    progress.set_size_request(280, -1);
    right.append(&progress);

    let elapsed = gtk::Label::new(Some("00:00"));
    elapsed.add_css_class("track-time");
    let separator = gtk::Label::new(Some(" / "));
    separator.add_css_class("track-time");
    let total = gtk::Label::new(Some("00:00"));
    total.add_css_class("track-time");

    let times = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    times.set_halign(gtk::Align::Center);
    times.append(&elapsed);
    times.append(&separator);
    times.append(&total);
    right.append(&times);

    let prev = icon_button("media-skip-backward-symbolic");
    let play = icon_button(ICON_PLAY);
    let next = icon_button("media-skip-forward-symbolic");

    let controls = gtk::Box::new(gtk::Orientation::Horizontal, 18);
    controls.set_halign(gtk::Align::Center);
    controls.append(&prev);
    controls.append(&play);
    controls.append(&next);
    right.append(&controls);

    let volume = gtk::Scale::with_range(gtk::Orientation::Horizontal, 0.0, 100.0, 1.0);
    volume.set_value(70.0);
    volume.set_draw_value(false);
    volume.add_css_class("music-scale");
    volume.set_size_request(180, -1);
    volume.set_halign(gtk::Align::Center);
    right.append(&volume);

    Widgets {
        window,
        search,
        menu_popover,
        menu_status,
        menu_items,
        list,
        row_popover,
        remove_button,
        repeat_all,
        repeat_one,
        shuffle,
        disc,
        title,
        counter,
        artist,
        progress,
        elapsed,
        total,
        prev,
        play,
        next,
        volume,
    }
}

// ---------------------------------------------------------------------------
// Nối tín hiệu
// ---------------------------------------------------------------------------

impl Dee {
    fn wire(self: &Rc<Self>) {
        let this = Rc::clone(self);

        // Bus GStreamer: hết bài / lỗi
        let guard = self.player.watch(
            {
                let weak = Rc::downgrade(self);
                move || {
                    if let Some(dee) = weak.upgrade() {
                        dee.on_eos();
                    }
                }
            },
            {
                let weak = Rc::downgrade(self);
                move |message| {
                    if let Some(dee) = weak.upgrade() {
                        dee.on_error(&message);
                    }
                }
            },
        );
        match guard {
            Ok(guard) => *self.bus_guard.borrow_mut() = Some(guard),
            Err(error) => eprintln!("Không theo dõi được bus GStreamer: {error}"),
        }

        // Đĩa than
        {
            let weak = Rc::downgrade(self);
            self.w.disc.set_draw_func(move |_, cr, w, h| {
                if let Some(dee) = weak.upgrade() {
                    draw_vinyl(cr, f64::from(w), f64::from(h), dee.theme.get(), dee.angle.get());
                }
            });
        }

        // Bộ đếm thời gian: đĩa quay (100 ms) và thanh tiến trình (250 ms)
        {
            let weak = Rc::downgrade(self);
            glib::timeout_add_local(Duration::from_millis(100), move || match weak.upgrade() {
                Some(dee) => {
                    dee.animate();
                    glib::ControlFlow::Continue
                }
                None => glib::ControlFlow::Break,
            });
        }
        {
            let weak = Rc::downgrade(self);
            glib::timeout_add_local(Duration::from_millis(250), move || match weak.upgrade() {
                Some(dee) => {
                    dee.tick();
                    glib::ControlFlow::Continue
                }
                None => glib::ControlFlow::Break,
            });
        }

        // Nút điều khiển
        self.w.play.connect_clicked(clone!(
            #[weak]
            this,
            move |_| this.toggle_play()
        ));
        self.w.prev.connect_clicked(clone!(
            #[weak]
            this,
            move |_| this.previous_track()
        ));
        self.w.next.connect_clicked(clone!(
            #[weak]
            this,
            move |_| this.next_track()
        ));

        // Tua
        self.w.progress.connect_value_changed(clone!(
            #[weak]
            this,
            move |scale| {
                if this.syncing.get() || this.duration.get() <= 0.0 {
                    return;
                }
                this.player.seek(scale.value());
            }
        ));

        // Âm lượng
        self.player.set_volume(0.7);
        self.w.volume.connect_value_changed(clone!(
            #[weak]
            this,
            move |scale| this.player.set_volume(scale.value() / 100.0)
        ));

        // Tìm kiếm
        self.w.search.connect_search_changed(clone!(
            #[weak]
            this,
            move |_| this.refresh_list()
        ));

        // Danh sách: nhấp đúp/Enter để phát, chuột phải để xóa
        self.w.list.connect_row_activated(clone!(
            #[weak]
            this,
            move |_, row| this.on_row_activated(row.index())
        ));

        let right_click = gtk::GestureClick::builder().button(3).build();
        right_click.connect_pressed(clone!(
            #[weak]
            this,
            move |_, _n_press, x, y| this.show_row_menu(x, y)
        ));
        self.w.list.add_controller(right_click);

        self.w.remove_button.connect_clicked(clone!(
            #[weak]
            this,
            move |_| {
                this.w.row_popover.popdown();
                let path = this.pending_remove.borrow_mut().take();
                if let Some(path) = path {
                    this.remove_song(&path);
                }
            }
        ));

        // Ba nút chế độ phát: bật cái này thì tắt hai cái còn lại
        let buttons = [
            self.w.repeat_all.clone(),
            self.w.repeat_one.clone(),
            self.w.shuffle.clone(),
        ];
        for (index, button) in buttons.iter().enumerate() {
            let all = buttons.clone();
            button.connect_toggled(move |current| {
                if current.is_active() {
                    for (j, other) in all.iter().enumerate() {
                        if j != index && other.is_active() {
                            other.set_active(false);
                        }
                    }
                }
                for (j, other) in all.iter().enumerate() {
                    update_mode_button(other, MODE_TIPS[j]);
                }
            });
        }

        // Menu chính
        let items = &self.w.menu_items;
        items[0].connect_clicked(clone!(
            #[weak]
            this,
            move |_| {
                this.w.menu_popover.popdown();
                this.choose_music_folder();
            }
        ));
        items[1].connect_clicked(clone!(
            #[weak]
            this,
            move |_| {
                this.w.menu_popover.popdown();
                this.show_theme_dialog();
            }
        ));
        items[2].connect_clicked(clone!(
            #[weak]
            this,
            move |_| this.set_status("Cài đặt — chức năng sẽ được bổ sung")
        ));
        items[3].connect_clicked(clone!(
            #[weak]
            this,
            move |_| {
                this.w.menu_popover.popdown();
                this.show_shortcuts();
            }
        ));
        items[4].connect_clicked(clone!(
            #[weak]
            this,
            move |_| {
                this.w.menu_popover.popdown();
                this.show_about();
            }
        ));

        // Phím tắt (giai đoạn capture để bắt được dù đang focus ở đâu)
        {
            let weak = Rc::downgrade(self);
            let keys = gtk::EventControllerKey::new();
            keys.set_propagation_phase(gtk::PropagationPhase::Capture);
            keys.connect_key_pressed(move |_, key, _code, state| match weak.upgrade() {
                Some(dee) => dee.on_key(key, state),
                None => glib::Propagation::Proceed,
            });
            self.w.window.add_controller(keys);
        }

        // Class "window-active" khi cửa sổ đang được focus
        self.w.window.connect_is_active_notify(|window| {
            if window.is_active() {
                window.add_css_class("window-active");
            } else {
                window.remove_css_class("window-active");
            }
        });

        // Dọn dẹp khi đóng. Closure này giữ một tham chiếu mạnh tới `Dee`
        // để đối tượng sống suốt vòng đời cửa sổ.
        let keep_alive = Rc::clone(self);
        self.w.window.connect_close_request(move |_| {
            keep_alive.shutdown();
            glib::Propagation::Proceed
        });
    }

    fn shutdown(&self) {
        self.player.stop();
        self.w.row_popover.unparent();
    }
}

// ---------------------------------------------------------------------------
// Logic phát nhạc
// ---------------------------------------------------------------------------

impl Dee {
    fn set_status(&self, text: &str) {
        self.w.menu_status.set_text(text);
    }

    fn current_index(&self) -> Option<usize> {
        let st = self.st.borrow();
        let current = st.now_playing.as_ref()?;
        st.files.iter().position(|path| path == current)
    }

    fn play_index(&self, index: usize) {
        let (path, idx) = {
            let mut st = self.st.borrow_mut();
            if st.files.is_empty() {
                drop(st);
                self.set_status("Hãy mở thư mục nhạc trước.");
                return;
            }
            let idx = index % st.files.len();
            let path = st.files[idx].clone();
            st.now_playing = Some(path.clone());
            (path, idx)
        };

        self.duration.set(0.0);
        self.player.play_file(&path);

        self.set_progress(0.0, 100.0);
        self.w.elapsed.set_text("00:00");
        self.w.total.set_text("00:00");
        self.w.title.set_text(&library::stem(&path));
        self.w.artist.set_text("Dee · Offline Music");
        self.w.play.set_icon_name(ICON_PAUSE);
        self.update_counter();
        self.select_row(Some(idx));
    }

    fn toggle_play(&self) {
        if self.st.borrow().now_playing.is_none() {
            self.play_index(0);
            return;
        }

        if self.player.is_playing() {
            self.player.set_playing(false);
            self.w.play.set_icon_name(ICON_PLAY);
        } else {
            self.player.set_playing(true);
            self.w.play.set_icon_name(ICON_PAUSE);
        }
    }

    fn previous_track(&self) {
        if self.st.borrow().files.is_empty() {
            return;
        }
        let current = self.current_index().unwrap_or(0);
        self.play_index(current.saturating_sub(1));
    }

    fn next_track(&self) {
        let len = self.st.borrow().files.len();
        if len == 0 {
            return;
        }
        if self.w.shuffle.is_active() {
            self.play_random();
            return;
        }
        let next = self.current_index().map_or(0, |i| i + 1);
        if next < len {
            self.play_index(next);
        } else if self.w.repeat_all.is_active() {
            self.play_index(0);
        }
    }

    fn play_random(&self) {
        let len = self.st.borrow().files.len();
        if len == 0 {
            return;
        }
        let pick = match self.current_index() {
            // Chọn đều trong các bài còn lại, không trùng bài hiện tại.
            Some(current) if len > 1 => {
                let mut r = glib::random_int_range(0, len as i32 - 1) as usize;
                if r >= current {
                    r += 1;
                }
                r
            }
            _ => glib::random_int_range(0, len as i32) as usize,
        };
        self.play_index(pick);
    }

    fn on_eos(&self) {
        let len = self.st.borrow().files.len();
        let current = self.current_index();

        if len > 0 {
            if self.w.repeat_one.is_active() {
                if let Some(i) = current {
                    self.play_index(i);
                    return;
                }
            } else if self.w.shuffle.is_active() {
                self.play_random();
                return;
            } else if self.w.repeat_all.is_active() {
                self.play_index(current.map_or(0, |i| (i + 1) % len));
                return;
            } else if let Some(i) = current {
                if i + 1 < len {
                    self.play_index(i + 1);
                    return;
                }
            }
        }

        self.player.stop();
        self.w.play.set_icon_name(ICON_PLAY);
    }

    fn on_error(&self, message: &str) {
        eprintln!("Lỗi phát nhạc: {message}");
        self.player.stop();
        self.w.play.set_icon_name(ICON_PLAY);
        self.w.artist.set_text("Không phát được tệp này");
    }

    fn reset_playback(&self, title: &str) {
        self.player.stop();
        self.st.borrow_mut().now_playing = None;
        self.duration.set(0.0);
        self.w.title.set_text(title);
        self.w.artist.set_text("Dee · Offline Music");
        self.w.elapsed.set_text("00:00");
        self.w.total.set_text("00:00");
        self.set_progress(0.0, 100.0);
        self.w.play.set_icon_name(ICON_PLAY);
    }

    fn animate(&self) {
        if self.player.is_playing() {
            // Một vòng quay mỗi 12 giây.
            self.angle.set((self.angle.get() + TAU / 12.0 / 10.0) % TAU);
            self.w.disc.queue_draw();
        }
    }

    fn tick(&self) {
        if self.st.borrow().now_playing.is_none() {
            return;
        }

        if let Some(duration) = self.player.duration() {
            if duration > 0.0 {
                self.duration.set(duration);
                self.w.total.set_text(&format_time(duration));
            }
        }

        if let Some(position) = self.player.position() {
            self.w.elapsed.set_text(&format_time(position));
            let duration = self.duration.get();
            if duration > 0.0 {
                self.set_progress(position.min(duration), duration);
            }
        }
    }

    /// Cập nhật thanh tiến trình mà không kích hoạt việc tua.
    fn set_progress(&self, value: f64, upper: f64) {
        self.syncing.set(true);
        self.w.progress.set_range(0.0, upper);
        self.w.progress.set_value(value);
        self.syncing.set(false);
    }
}

// ---------------------------------------------------------------------------
// Danh sách nhạc
// ---------------------------------------------------------------------------

impl Dee {
    fn restore_music_folder(&self) {
        if let Some(folder) = config::get_string("music_folder") {
            let folder = PathBuf::from(folder);
            if folder.is_dir() {
                self.show_music_files(library::scan(&folder));
            }
        }
    }

    fn choose_music_folder(self: &Rc<Self>) {
        let dialog = gtk::FileDialog::builder()
            .title("Chọn thư mục nhạc")
            .accept_label("Chọn thư mục")
            .modal(true)
            .build();

        let this = Rc::clone(self);
        dialog.select_folder(
            Some(&self.w.window),
            gio::Cancellable::NONE,
            move |result| {
                if let Ok(folder) = result {
                    if let Some(path) = folder.path() {
                        config::set_string("music_folder", &path.to_string_lossy());
                        this.show_music_files(library::scan(&path));
                    }
                }
            },
        );
    }

    fn show_music_files(&self, files: Vec<PathBuf>) {
        self.reset_playback("Chọn một bài hát");
        self.st.borrow_mut().all = files;
        self.refresh_list();
    }

    fn refresh_list(&self) {
        let query = self.w.search.text().trim().to_lowercase();

        let filtered: Vec<PathBuf> = {
            let st = self.st.borrow();
            st.all
                .iter()
                .filter(|path| {
                    query.is_empty() || library::stem(path).to_lowercase().contains(&query)
                })
                .cloned()
                .collect()
        };

        self.render_list(&filtered);
        self.st.borrow_mut().files = filtered;
        self.update_counter();

        let current = self.current_index();
        self.select_row(current);
    }

    fn render_list(&self, files: &[PathBuf]) {
        while let Some(row) = self.w.list.row_at_index(0) {
            self.w.list.remove(&row);
        }

        if files.is_empty() {
            let label = gtk::Label::new(Some("Không tìm thấy bài hát phù hợp"));
            label.set_xalign(0.0);
            label.set_margin_top(10);
            label.set_margin_bottom(10);
            label.set_margin_start(10);

            let row = gtk::ListBoxRow::new();
            row.set_activatable(false);
            row.set_selectable(false);
            row.set_child(Some(&label));
            self.w.list.append(&row);
            return;
        }

        for path in files {
            let label = gtk::Label::new(Some(&library::stem(path)));
            label.set_xalign(0.0);
            label.set_ellipsize(pango::EllipsizeMode::End);
            label.set_margin_top(8);
            label.set_margin_bottom(8);
            label.set_margin_start(10);
            label.set_margin_end(10);
            label.set_tooltip_text(Some(&path.to_string_lossy()));

            let row = gtk::ListBoxRow::new();
            row.set_child(Some(&label));
            self.w.list.append(&row);
        }
    }

    fn update_counter(&self) {
        let count = self.st.borrow().files.len();
        let searching = !self.w.search.text().trim().is_empty();
        let text = match (self.current_index(), searching) {
            (Some(i), _) => format!("{}/{}", i + 1, count),
            (None, true) => format!("{count} bài"),
            (None, false) => format!("—/{count}"),
        };
        self.w.counter.set_text(&text);
    }

    fn select_row(&self, index: Option<usize>) {
        match index.and_then(|i| self.w.list.row_at_index(i as i32)) {
            Some(row) => self.w.list.select_row(Some(&row)),
            None => self.w.list.unselect_all(),
        }
    }

    fn on_row_activated(&self, index: i32) {
        if index < 0 {
            return;
        }
        let index = index as usize;
        if index < self.st.borrow().files.len() {
            self.play_index(index);
        }
    }

    fn show_row_menu(&self, x: f64, y: f64) {
        let Some(row) = self.w.list.row_at_y(y as i32) else {
            return;
        };
        let index = row.index();
        if index < 0 {
            return;
        }
        let path = self.st.borrow().files.get(index as usize).cloned();
        let Some(path) = path else {
            return;
        };

        *self.pending_remove.borrow_mut() = Some(path);
        self.w
            .row_popover
            .set_pointing_to(Some(&gdk::Rectangle::new(x as i32, y as i32, 1, 1)));
        self.w.row_popover.popup();
    }

    /// Xóa bài khỏi danh sách (không xóa tệp trên đĩa).
    fn remove_song(&self, path: &Path) {
        let is_current = self.st.borrow().now_playing.as_deref() == Some(path);
        if is_current {
            // Dừng phát để không giữ tệp đang dùng.
            self.reset_playback("Chưa phát bài nào");
        }
        self.st.borrow_mut().all.retain(|p| p != path);
        self.refresh_list();
    }
}

// ---------------------------------------------------------------------------
// Giao diện màu, phím tắt, hộp thoại
// ---------------------------------------------------------------------------

impl Dee {
    fn apply_theme(&self, theme: Theme, save: bool) {
        self.css.load_from_string(&theme.css(BASE_CSS));
        self.theme.set(theme);
        self.w.disc.queue_draw();
        if save {
            config::set_string("theme", theme.name());
        }
    }

    fn search_has_focus(&self) -> bool {
        // Ô tìm kiếm có widget con nhận focus thật sự nên cần kiểm tra cả cây con.
        match gtk::prelude::RootExt::focus(&self.w.window) {
            Some(focus) => {
                focus.is_ancestor(&self.w.search)
                    || focus == self.w.search.clone().upcast::<gtk::Widget>()
            }
            None => false,
        }
    }

    fn on_key(self: &Rc<Self>, key: gdk::Key, state: gdk::ModifierType) -> glib::Propagation {
        if state.contains(gdk::ModifierType::CONTROL_MASK) {
            if key == gdk::Key::o || key == gdk::Key::O {
                self.choose_music_folder();
                return glib::Propagation::Stop;
            }
            if key == gdk::Key::m || key == gdk::Key::M {
                self.w.menu_popover.popup();
                return glib::Propagation::Stop;
            }
            if key == gdk::Key::q || key == gdk::Key::Q {
                if let Some(app) = self.w.window.application() {
                    app.quit();
                }
                return glib::Propagation::Stop;
            }
            return glib::Propagation::Proceed;
        }

        // Khi đang gõ tìm kiếm, nhường Space và phím mũi tên cho ô nhập.
        if self.search_has_focus() {
            return glib::Propagation::Proceed;
        }

        if key == gdk::Key::space {
            self.toggle_play();
        } else if key == gdk::Key::Left {
            self.previous_track();
        } else if key == gdk::Key::Right {
            self.next_track();
        } else if key == gdk::Key::Up {
            let value = (self.w.volume.value() + 5.0).min(100.0);
            self.w.volume.set_value(value);
        } else if key == gdk::Key::Down {
            let value = (self.w.volume.value() - 5.0).max(0.0);
            self.w.volume.set_value(value);
        } else {
            return glib::Propagation::Proceed;
        }
        glib::Propagation::Stop
    }

    fn show_theme_dialog(self: &Rc<Self>) {
        let (win, content) = modal_window(&self.w.window, "Giao diện Dee", 440, 150);
        content.append(&dialog_label("Chọn giao diện bạn muốn sử dụng:", true));

        let row = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        row.set_halign(gtk::Align::Center);
        content.append(&row);

        let this = Rc::clone(self);
        for theme in Theme::ALL {
            let button = gtk::Button::with_label(theme.name());
            button.set_hexpand(true);
            button.connect_clicked(clone!(
                #[weak]
                this,
                #[weak]
                win,
                move |_| {
                    this.apply_theme(theme, true);
                    this.set_status(&format!("Đã áp dụng giao diện {}.", theme.name()));
                    win.close();
                }
            ));
            row.append(&button);
        }

        win.present();
    }

    fn show_shortcuts(&self) {
        let (win, content) = modal_window(&self.w.window, "Dee - Keyboard Shortcuts", 420, 300);

        let rows = [
            ("SPACE", "Play / Pause"),
            ("LEFT", "Previous track"),
            ("RIGHT", "Next track"),
            ("UP", "Volume up 5%"),
            ("DOWN", "Volume down 5%"),
            ("CTRL+O", "Open music folder"),
            ("CTRL+M", "Open main menu"),
            ("CTRL+Q", "Quit Dee"),
        ];

        let grid = gtk::Grid::new();
        grid.set_row_spacing(8);
        grid.set_column_spacing(18);
        for (i, (keys, description)) in rows.iter().enumerate() {
            let key_label = dialog_label(keys, true);
            key_label.set_size_request(90, -1);
            grid.attach(&key_label, 0, i as i32, 1, 1);

            let description_label = dialog_label(description, false);
            description_label.set_hexpand(true);
            grid.attach(&description_label, 1, i as i32, 1, 1);
        }
        content.append(&grid);
        content.append(&close_button(&win));

        win.present();
    }

    fn show_about(&self) {
        let (win, content) = modal_window(&self.w.window, "Giới thiệu Dee", 420, 220);

        let text = "Dee là trình phát nhạc ngoại tuyến đơn giản, \
                    giúp bạn thưởng thức âm nhạc mọi lúc, không cần Internet.\n\n\
                    Dee is a simple offline music player for enjoying your music \
                    anytime, without the internet.\n\n\
                    Version 1.0";
        let label = dialog_label(text, false);
        label.set_selectable(true);
        label.set_max_width_chars(52);
        content.append(&label);
        content.append(&close_button(&win));

        win.present();
    }
}

fn modal_window(
    parent: &gtk::ApplicationWindow,
    title: &str,
    width: i32,
    height: i32,
) -> (gtk::Window, gtk::Box) {
    let win = gtk::Window::builder()
        .title(title)
        .transient_for(parent)
        .modal(true)
        .default_width(width)
        .default_height(height)
        .build();

    let content = gtk::Box::new(gtk::Orientation::Vertical, 12);
    content.set_margin_top(20);
    content.set_margin_bottom(20);
    content.set_margin_start(22);
    content.set_margin_end(22);
    win.set_child(Some(&content));

    (win, content)
}

fn dialog_label(text: &str, bold: bool) -> gtk::Label {
    let label = gtk::Label::new(Some(text));
    label.set_xalign(0.0);
    label.set_wrap(true);
    label.add_css_class(if bold { "dialog-key" } else { "dialog-text" });
    label
}

fn close_button(win: &gtk::Window) -> gtk::Button {
    let button = gtk::Button::with_label("Đóng");
    button.set_halign(gtk::Align::End);
    button.connect_clicked(clone!(
        #[weak]
        win,
        move |_| win.close()
    ));
    button
}

// ---------------------------------------------------------------------------
// Vẽ đĩa than
// ---------------------------------------------------------------------------

fn format_time(seconds: f64) -> String {
    let total = seconds.max(0.0) as u64;
    format!("{:02}:{:02}", total / 60, total % 60)
}

fn set_rgb(cr: &cairo::Context, color: Rgb) {
    cr.set_source_rgb(color.0, color.1, color.2);
}

fn draw_small_vinyl(cr: &cairo::Context, width: f64, height: f64) {
    let cx = width / 2.0;
    let cy = height / 2.0;
    let radius = width.min(height) * 0.38;

    cr.arc(cx, cy, radius, 0.0, TAU);
    cr.set_source_rgb(0.12, 0.13, 0.11);
    let _ = cr.fill();

    for ratio in [0.82, 0.63, 0.44] {
        cr.arc(cx, cy, radius * ratio, 0.0, TAU);
        cr.set_line_width(0.7);
        cr.set_source_rgb(0.36, 0.39, 0.32);
        let _ = cr.stroke();
    }

    cr.arc(cx, cy, radius * 0.29, 0.0, TAU);
    cr.set_source_rgb(0.73, 0.80, 0.63);
    let _ = cr.fill();

    cr.arc(cx, cy, radius * 0.055, 0.0, TAU);
    cr.set_source_rgb(0.12, 0.13, 0.11);
    let _ = cr.fill();
}

fn draw_vinyl(cr: &cairo::Context, width: f64, height: f64, theme: Theme, angle: f64) {
    let radius = width.min(height) * 0.43;
    let cx = width / 2.0;
    let cy = height / 2.0;
    let colors = theme.vinyl_colors();

    let _ = cr.save();
    cr.translate(cx, cy);
    cr.rotate(angle);
    cr.translate(-cx, -cy);

    cr.arc(cx, cy, radius, 0.0, TAU);
    set_rgb(cr, colors.disc);
    let _ = cr.fill();

    for ratio in [0.88, 0.76, 0.64, 0.52] {
        cr.arc(cx, cy, radius * ratio, 0.0, TAU);
        cr.set_line_width(1.0);
        set_rgb(cr, colors.groove);
        let _ = cr.stroke();
    }

    cr.arc(cx, cy, radius * 0.32, 0.0, TAU);
    set_rgb(cr, colors.label);
    let _ = cr.fill();

    cr.arc(cx, cy, radius * 0.045, 0.0, TAU);
    set_rgb(cr, colors.disc);
    let _ = cr.fill();

    // Vạch sáng nhỏ giúp nhìn thấy đĩa than đang quay.
    cr.arc(cx, cy, radius * 0.91, 0.15, 0.38);
    cr.set_line_width(3.0);
    set_rgb(cr, colors.highlight);
    let _ = cr.stroke();

    let _ = cr.restore();
}
