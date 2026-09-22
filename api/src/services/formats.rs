//! Format catalogue (§9.2). Editable in the database: platforms change their
//! formats, and that must never require a code change.

pub struct FormatDef {
    pub key: &'static str,
    pub platform: &'static str,
    pub label: &'static str,
    pub width: i32,
    pub height: i32,
    pub kind: &'static str,
}

pub const CATALOG: &[FormatDef] = &[
    FormatDef {
        key: "ig_square",
        platform: "Instagram",
        label: "Carre",
        width: 1080,
        height: 1080,
        kind: "image",
    },
    FormatDef {
        key: "ig_portrait",
        platform: "Instagram",
        label: "Portrait (recommande)",
        width: 1080,
        height: 1350,
        kind: "image",
    },
    FormatDef {
        key: "ig_story",
        platform: "Instagram",
        label: "Story / Reel",
        width: 1080,
        height: 1920,
        kind: "image",
    },
    FormatDef {
        key: "reel_9_16",
        platform: "Instagram",
        label: "Reel",
        width: 1080,
        height: 1920,
        kind: "video",
    },
    FormatDef {
        key: "tiktok_9_16",
        platform: "TikTok",
        label: "Video / image",
        width: 1080,
        height: 1920,
        kind: "video",
    },
    FormatDef {
        key: "yt_thumbnail",
        platform: "YouTube",
        label: "Miniature",
        width: 1280,
        height: 720,
        kind: "image",
    },
    FormatDef {
        key: "yt_shorts",
        platform: "YouTube",
        label: "Shorts",
        width: 1080,
        height: 1920,
        kind: "video",
    },
    FormatDef {
        key: "yt_banner",
        platform: "YouTube",
        label: "Banniere de chaine",
        width: 2560,
        height: 1440,
        kind: "image",
    },
    FormatDef {
        key: "fb_post",
        platform: "Facebook",
        label: "Publication",
        width: 1080,
        height: 1080,
        kind: "image",
    },
    FormatDef {
        key: "fb_wide",
        platform: "Facebook",
        label: "Publication large",
        width: 1920,
        height: 1080,
        kind: "image",
    },
    // Poster: high-resolution PDF export, RGB only (CMYK is out of scope).
    FormatDef {
        key: "poster_a3",
        platform: "Affiche",
        label: "A3",
        width: 3508,
        height: 4961,
        kind: "print",
    },
    FormatDef {
        key: "poster_a4",
        platform: "Affiche",
        label: "A4",
        width: 2480,
        height: 3508,
        kind: "print",
    },
];

/// The ratio is **derived** from the resolution: it is never entered
/// separately, which makes a frame inconsistent with its format impossible
/// (§9.2).
pub fn ratio(width: i32, height: i32) -> f64 {
    width as f64 / height as f64
}

pub fn ratio_label(width: i32, height: i32) -> String {
    let g = gcd(width.unsigned_abs(), height.unsigned_abs()).max(1) as i32;
    format!("{}:{}", width / g, height / g)
}

fn gcd(a: u32, b: u32) -> u32 {
    if b == 0 {
        a
    } else {
        gcd(b, a % b)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_ratios_the_prd_announces_are_respected() {
        assert_eq!(ratio_label(1080, 1080), "1:1");
        assert_eq!(ratio_label(1080, 1350), "4:5");
        assert_eq!(ratio_label(1080, 1920), "9:16");
        assert_eq!(ratio_label(1280, 720), "16:9");
        assert_eq!(ratio_label(2560, 1440), "16:9");
    }
}
