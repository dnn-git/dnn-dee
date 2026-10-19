//! Ba giao diện màu: Xanh Mint (mặc định), Hồng Anh Đào, Sói Xám.
//! Cách đổi màu giống bản Python: thay các mã #RRGGBB trong CSS gốc
//! bằng mã tương ứng của bảng màu được chọn.

pub type Rgb = (f64, f64, f64);

pub struct VinylColors {
    pub disc: Rgb,
    pub groove: Rgb,
    pub label: Rgb,
    pub highlight: Rgb,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Theme {
    Mint,
    Cherry,
    Wolf,
}

type Palette = &'static [(&'static str, &'static str)];

const WOLF: Palette = &[
    ("#E9EEDF", "#3D3D3D"),
    ("#C8D9B9", "#555555"),
    ("#C6D4BB", "#5E5E5E"),
    ("#263323", "#F0F0F0"),
    ("#52604D", "#D0D0D0"),
    ("#E2EBDD", "#414141"),
    ("#D5E1CE", "#484848"),
    ("#718568", "#777777"),
    ("#FFFFFF", "#FFFFFF"),
    ("#E8EEE2", "#484848"),
    ("#293329", "#F0F0F0"),
    ("#DCE8D3", "#505050"),
    ("#D2DDC8", "#555555"),
    ("#AAB9A0", "#666666"),
    ("#F5F7F0", "#414141"),
    ("#EEF2E8", "#484848"),
    ("#E1EAD9", "#555555"),
    ("#34452F", "#DADADA"),
    ("#7D8375", "#B5B5B5"),
    ("#C94C4C", "#C94C4C"),
    ("#202020", "#F0F0F0"),
    ("#52664B", "#707070"),
    ("#263326", "#F0F0F0"),
    ("#000000", "#F0F0F0"),
];

const CHERRY: Palette = &[
    ("#E9EEDF", "#F9EDF2"),
    ("#C8D9B9", "#F0C9D9"),
    ("#C6D4BB", "#E9C1D2"),
    ("#263323", "#553746"),
    ("#52604D", "#896879"),
    ("#E2EBDD", "#F7E4EC"),
    ("#D5E1CE", "#F2D6E2"),
    ("#718568", "#C47D9D"),
    ("#FFFFFF", "#FFFFFF"),
    ("#E8EEE2", "#F8E5ED"),
    ("#293329", "#553746"),
    ("#DCE8D3", "#F0D2E0"),
    ("#D2DDC8", "#EBC6D7"),
    ("#AAB9A0", "#DDB4C7"),
    ("#F5F7F0", "#FFF8FB"),
    ("#EEF2E8", "#FCEEF3"),
    ("#E1EAD9", "#F5DDE7"),
    ("#34452F", "#70485B"),
    ("#7D8375", "#9D7F8E"),
    ("#C94C4C", "#B94D70"),
    ("#202020", "#553746"),
];

impl Theme {
    pub const ALL: [Theme; 3] = [Theme::Mint, Theme::Cherry, Theme::Wolf];

    pub fn name(self) -> &'static str {
        match self {
            Theme::Mint => "Xanh Mint",
            Theme::Cherry => "Hồng Anh Đào",
            Theme::Wolf => "Sói Xám",
        }
    }

    /// Chấp nhận cả tên cũ ("Dark xám", "Hồng nhạt") trong cấu hình.
    pub fn from_name(name: &str) -> Theme {
        match name {
            "Hồng Anh Đào" | "Hồng nhạt" => Theme::Cherry,
            "Sói Xám" | "Dark xám" => Theme::Wolf,
            _ => Theme::Mint,
        }
    }

    fn palette(self) -> Palette {
        match self {
            Theme::Mint => &[],
            Theme::Cherry => CHERRY,
            Theme::Wolf => WOLF,
        }
    }

    /// CSS đã đổi màu theo giao diện này.
    pub fn css(self, base: &str) -> String {
        recolor(base, self.palette())
    }

    pub fn vinyl_colors(self) -> VinylColors {
        match self {
            Theme::Cherry => VinylColors {
                disc: (0.941, 0.788, 0.851),
                groove: (0.72, 0.55, 0.62),
                label: (0.97, 0.86, 0.90),
                highlight: (1.0, 0.94, 0.97),
            },
            Theme::Wolf => VinylColors {
                disc: (0.53, 0.53, 0.53),
                groove: (0.38, 0.38, 0.38),
                label: (0.72, 0.72, 0.72),
                highlight: (0.82, 0.82, 0.82),
            },
            Theme::Mint => VinylColors {
                disc: (0.50, 0.58, 0.47),
                groove: (0.36, 0.39, 0.33),
                label: (0.78, 0.83, 0.69),
                highlight: (0.82, 0.87, 0.72),
            },
        }
    }
}

/// Thay mọi mã `#RRGGBB` có trong bảng màu (một lượt, không thay chồng).
fn recolor(css: &str, palette: Palette) -> String {
    if palette.is_empty() {
        return css.to_owned();
    }

    let mut out = String::with_capacity(css.len());
    let mut rest = css;

    while let Some(pos) = rest.find('#') {
        out.push_str(&rest[..pos]);
        let after = &rest[pos + 1..];

        let is_hex6 = after.len() >= 6
            && after.as_bytes()[..6].iter().all(|b| b.is_ascii_hexdigit());

        if is_hex6 {
            let hex = &rest[pos..pos + 7];
            let upper = hex.to_ascii_uppercase();
            match palette.iter().find(|(from, _)| *from == upper) {
                Some((_, to)) => out.push_str(to),
                None => out.push_str(hex),
            }
            rest = &rest[pos + 7..];
        } else {
            out.push('#');
            rest = after;
        }
    }

    out.push_str(rest);
    out
}
