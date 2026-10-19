# Dee Offline Music (bản Rust)

Trình phát nhạc ngoại tuyến viết lại từ `app.py` sang Rust, dùng GTK4 (`gtk4-rs`) và GStreamer (`gstreamer-rs`).

## Cài môi trường (Ubuntu 26.04)

```bash
sudo apt update
sudo apt install build-essential pkg-config cargo \
  libgtk-4-dev libgstreamer1.0-dev libgstreamer-plugins-base1.0-dev \
  gstreamer1.0-plugins-base gstreamer1.0-plugins-good \
  gstreamer1.0-plugins-bad gstreamer1.0-plugins-ugly gstreamer1.0-libav
```

(`plugins-good/bad/ugly` + `libav` để phát đủ mp3, flac, m4a, aac, opus, wma.)

## Build và chạy

```bash
cargo run --release          # chạy thử
cargo build --release        # file chạy: target/release/dee
```

## Cài vào menu ứng dụng (tuỳ chọn)

```bash
cargo build --release
sudo install -Dm755 target/release/dee /usr/local/bin/dee
sudo install -Dm644 vn.dnn.Dee.desktop /usr/share/applications/vn.dnn.Dee.desktop
```

## Cấu trúc

| File | Vai trò |
|---|---|
| `src/main.rs` | Điểm vào, khởi tạo GStreamer + GTK |
| `src/app.rs` | Giao diện, phím tắt, hộp thoại, logic phát nhạc |
| `src/player.rs` | Bọc `playbin` của GStreamer |
| `src/theme.rs` | 3 giao diện màu (Xanh Mint, Hồng Anh Đào, Sói Xám) |
| `src/config.rs` | Đọc/ghi `~/.config/dee/config.json` (dùng lại được cấu hình cũ) |
| `src/library.rs` | Quét thư mục nhạc |
| `src/style.css` | CSS lấy nguyên văn từ `app.py` |

## Khác biệt so với bản Python

- Kéo cửa sổ bằng `GtkWindowHandle` (ổn định hơn trên Wayland). Cửa sổ vẫn cố định kích thước như bản gốc, nên bỏ các vùng kéo mép (vốn không có tác dụng khi `resizable = false`) và dòng in kích thước ra terminal.
- Bài đang phát được nhớ theo đường dẫn, nên tìm kiếm hoặc xóa bài không làm lệch bài đang phát.
- Khi đang gõ ở ô tìm kiếm, Space và phím mũi tên dành cho ô nhập thay vì bị phím tắt chiếm.
- Chọn thư mục mới sẽ dừng bài đang phát; tệp có ảnh bìa/video không mở cửa sổ video.
- Hộp thoại dùng class CSS theo giao diện màu nên đọc được cả ở "Sói Xám".
- "Xóa" ở menu chuột phải chỉ xóa khỏi danh sách, không xóa tệp trên đĩa (giống bản gốc).
