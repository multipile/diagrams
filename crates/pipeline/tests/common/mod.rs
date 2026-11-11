//! What the end-to-end tests share: the cases, and how to look at what the
//! pipeline made of them.

#![allow(dead_code)]

use std::path::{Path, PathBuf};

use diagram_pipeline::{Diagram, Renderer};

/// One diagram file of `cases/`.
pub struct Case {
    /// `<type>/<name>`, as the test names it.
    pub name: String,
    pub path: PathBuf,
    pub source: String,
}

impl Case {
    pub fn diagram(&self) -> Diagram {
        diagram_pipeline::read(&self.source)
            .unwrap_or_else(|error| panic!("{}:\n{error}", self.name))
            .diagram
    }

    /// Writes what the pipeline made of the case where a person can look
    /// at it: `target/tmp/e2e/<type>/<name>.<extension>`.
    pub fn keep(&self, extension: &str, bytes: &[u8]) {
        let path = out_dir().join(format!("{}.{extension}", self.name));
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, bytes).unwrap();
    }
}

pub fn cases_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("cases")
}

pub fn out_dir() -> PathBuf {
    Path::new(env!("CARGO_TARGET_TMPDIR")).join("e2e")
}

/// Every case in `cases/<dir>/`, by name.
pub fn cases(dir: &str) -> Vec<Case> {
    let mut paths: Vec<PathBuf> = std::fs::read_dir(cases_dir().join(dir))
        .unwrap_or_else(|error| panic!("cases/{dir}: {error}"))
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|e| e == "diagram"))
        .collect();
    paths.sort();
    paths
        .into_iter()
        .map(|path| Case {
            name: format!("{dir}/{}", path.file_stem().unwrap().to_string_lossy()),
            source: std::fs::read_to_string(&path).unwrap(),
            path,
        })
        .collect()
}

pub const TYPES: [&str; 9] = [
    "architecture",
    "sequence",
    "state",
    "swimlane",
    "process",
    "tree",
    "dependency",
    "nested",
    "treemap",
];

/// Every case of every type.
pub fn all_cases() -> Vec<Case> {
    TYPES.iter().flat_map(|kind| cases(kind)).collect()
}

/// A renderer at the page's own size, which is quicker to draw and to
/// check than the default's twice.
pub fn renderer() -> Renderer {
    Renderer::new().with_scale(1.0)
}

/// A picture decoded to RGBA.
pub struct Picture {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

impl Picture {
    pub fn png(bytes: &[u8]) -> Picture {
        let mut decoder = png::Decoder::new(std::io::Cursor::new(bytes));
        decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::ALPHA);
        let mut reader = decoder.read_info().expect("a PNG");
        let mut buffer = vec![0; reader.output_buffer_size().unwrap()];
        let info = reader.next_frame(&mut buffer).expect("a frame");
        let channels = info.line_size / info.width as usize;
        let mut rgba = Vec::with_capacity(info.width as usize * info.height as usize * 4);
        for pixel in buffer[..info.line_size * info.height as usize].chunks(channels) {
            match channels {
                4 => rgba.extend_from_slice(pixel),
                3 => rgba.extend_from_slice(&[pixel[0], pixel[1], pixel[2], 255]),
                2 => rgba.extend_from_slice(&[pixel[0], pixel[0], pixel[0], pixel[1]]),
                _ => rgba.extend_from_slice(&[pixel[0], pixel[0], pixel[0], 255]),
            }
        }
        Picture {
            width: info.width,
            height: info.height,
            rgba,
        }
    }

    pub fn pixels(&self) -> impl Iterator<Item = [u8; 4]> + '_ {
        self.rgba.chunks(4).map(|p| [p[0], p[1], p[2], p[3]])
    }

    /// How many pixels are not the paper: something is drawn there.
    pub fn inked(&self) -> usize {
        self.pixels().filter(|p| !near(*p, [0xf5, 0xf5, 0xf5])).count()
    }

    /// How many pixels are near the accent colour.
    pub fn accented(&self) -> usize {
        self.pixels()
            .filter(|p| p[0] > 150 && p[0] as i32 - p[2] as i32 > 80 && p[0] as i32 - p[1] as i32 > 60)
            .count()
    }

    pub fn at(&self, x: u32, y: u32) -> [u8; 4] {
        let i = (y * self.width + x) as usize * 4;
        [self.rgba[i], self.rgba[i + 1], self.rgba[i + 2], self.rgba[i + 3]]
    }
}

fn near(a: [u8; 4], b: [u8; 3]) -> bool {
    (0..3).all(|i| (a[i] as i32 - b[i] as i32).abs() <= 6)
}

/// The frames of a GIF, each decoded to RGBA, and how long each shows.
pub fn gif_frames(bytes: &[u8]) -> Vec<(Vec<u8>, u16)> {
    let mut options = gif::DecodeOptions::new();
    options.set_color_output(gif::ColorOutput::RGBA);
    let mut decoder = options.read_info(std::io::Cursor::new(bytes)).expect("a GIF");
    let mut frames = Vec::new();
    while let Some(frame) = decoder.read_next_frame().expect("a frame") {
        frames.push((frame.buffer.to_vec(), frame.delay));
    }
    frames
}
