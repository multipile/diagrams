//! The fonts a diagram is drawn in: Geist for names and headings, and Geist
//! Mono for second lines, labels and tags.
//!
//! Both are by the Geist Project Authors, under the SIL Open Font License
//! 1.1, which `fonts/OFL.txt` holds. A page embeds them, so it looks the
//! same in any browser, and the renderer registers them, since takumi loads
//! no system fonts.

/// A font file, and the family a page names it by.
#[derive(Debug, Clone, Copy)]
pub struct Font {
    pub family: &'static str,
    /// A variable WOFF2 font, covering every weight.
    pub woff2: &'static [u8],
}

pub const GEIST: Font = Font {
    family: "Geist",
    woff2: include_bytes!("../fonts/Geist.woff2"),
};

pub const GEIST_MONO: Font = Font {
    family: "Geist Mono",
    woff2: include_bytes!("../fonts/GeistMono.woff2"),
};

pub const FONTS: [Font; 2] = [GEIST, GEIST_MONO];

/// The licence the fonts are under.
pub const LICENSE: &str = include_str!("../fonts/OFL.txt");
