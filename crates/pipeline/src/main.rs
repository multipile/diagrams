//! `diagram`: reads, builds and renders diagram files, to look at what the
//! pipeline makes.

use std::path::Path;
use std::process::ExitCode;

use diagram_pipeline::{Animation, MotionFormat, Renderer, StillFormat};

const USAGE: &str = "\
usage:
  diagram check <file>...                      read each file, and say what is wrong
  diagram render <file> -o <out> [--reveal]    a page (.html) or a picture (.png .webp),
                                               or the diagram revealed (.gif .apng, or .webp with --reveal)
  diagram diff <before> <after> -o <out>       the two versions as a page or a picture, changes marked,
                                               or the first becoming the second (.gif .apng)
options:
  --fps <n>       frames a second of an animation (default 25)
  --scale <n>     pixels of a picture for each pixel of the page (default 2)
  --at <ms>       a picture of the animation at that moment, not of its end
  --changed <piles>  mark the boxes of these piles changed, as the file names them: @cli,@storage";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("{message}");
            ExitCode::FAILURE
        }
    }
}

struct Flags {
    files: Vec<String>,
    out: Option<String>,
    reveal: bool,
    fps: u32,
    scale: f32,
    at: Option<u32>,
    /// The piles to mark changed, as the file names them.
    changed: Vec<String>,
}

fn flags(args: &[String]) -> Result<Flags, String> {
    let mut flags = Flags {
        files: Vec::new(),
        out: None,
        reveal: false,
        fps: 25,
        scale: 2.0,
        at: None,
        changed: Vec::new(),
    };
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        let mut value = |name: &str| args.next().cloned().ok_or(format!("{name} needs a value\n\n{USAGE}"));
        match arg.as_str() {
            "-o" | "--out" => flags.out = Some(value("-o")?),
            "--reveal" => flags.reveal = true,
            "--fps" => flags.fps = value("--fps")?.parse().map_err(|_| "--fps is a number".to_string())?,
            "--changed" => {
                flags.changed = value("--changed")?
                    .split(',')
                    .map(|pile| pile.trim().to_string())
                    .filter(|pile| !pile.is_empty())
                    .collect()
            }
            "--at" => {
                flags.at = Some(
                    value("--at")?
                        .parse()
                        .map_err(|_| "--at is a number of milliseconds".to_string())?,
                )
            }
            "--scale" => {
                flags.scale = value("--scale")?
                    .parse()
                    .map_err(|_| "--scale is a number".to_string())?
            }
            other if other.starts_with('-') => return Err(format!("no option {other}\n\n{USAGE}")),
            file => flags.files.push(file.to_string()),
        }
    }
    Ok(flags)
}

fn read(path: &str) -> Result<String, String> {
    std::fs::read_to_string(path).map_err(|error| format!("{path}: {error}"))
}

/// What a file name asks for.
enum Out {
    Html,
    Still(StillFormat),
    Motion(MotionFormat),
}

fn out_kind(path: &str, reveal: bool) -> Result<Out, String> {
    let extension = Path::new(path).extension().and_then(|e| e.to_str()).unwrap_or("");
    match (extension, reveal) {
        ("html", _) => Ok(Out::Html),
        ("png", _) => Ok(Out::Still(StillFormat::Png)),
        ("webp", false) => Ok(Out::Still(StillFormat::WebP)),
        ("webp", true) => Ok(Out::Motion(MotionFormat::WebP)),
        ("gif", _) => Ok(Out::Motion(MotionFormat::Gif)),
        ("apng", _) => Ok(Out::Motion(MotionFormat::Apng)),
        _ => Err(format!("{path}: write .html, .png, .webp, .gif or .apng")),
    }
}

fn run(args: &[String]) -> Result<(), String> {
    let Some((command, rest)) = args.split_first() else {
        return Err(USAGE.to_string());
    };
    let flags = flags(rest)?;
    let renderer = Renderer::new().with_scale(flags.scale);
    let fail = |path: &str, error: diagram_pipeline::Error| format!("{path}:\n{error}");
    match command.as_str() {
        "check" => {
            if flags.files.is_empty() {
                return Err(USAGE.to_string());
            }
            let mut wrong = 0;
            for path in &flags.files {
                match diagram_pipeline::read(&read(path)?) {
                    Ok(read) => println!("{path}: {}, read", read.diagram.kind().word()),
                    Err(error) => {
                        wrong += 1;
                        eprintln!("{}", fail(path, error));
                    }
                }
            }
            if wrong > 0 {
                return Err(format!("{wrong} of {} files have findings", flags.files.len()));
            }
            Ok(())
        }
        "render" => {
            let [path] = flags.files.as_slice() else {
                return Err(USAGE.to_string());
            };
            let out = flags.out.as_deref().ok_or(USAGE)?;
            let source = read(path)?;
            if !flags.changed.is_empty() {
                let changed: Vec<&str> = flags.changed.iter().map(String::as_str).collect();
                let document =
                    diagram_pipeline::changed_document(&source, &changed).map_err(|error| fail(path, error))?;
                let bytes = match out_kind(out, false)? {
                    Out::Html => {
                        diagram_pipeline::html::page(&document, &diagram_pipeline::Options::default()).into_bytes()
                    }
                    Out::Still(format) => renderer
                        .still(&document, format, None)
                        .map_err(|error| fail(path, error.into()))?,
                    Out::Motion(_) => {
                        return Err(format!(
                            "{out}: a diagram with its changed piles marked is still: write .html, .png or .webp"
                        ));
                    }
                };
                return std::fs::write(out, bytes).map_err(|error| format!("{out}: {error}"));
            }
            let bytes = match out_kind(out, flags.reveal)? {
                Out::Html => {
                    let animation = if flags.reveal {
                        Animation::Reveal
                    } else {
                        Animation::None
                    };
                    diagram_pipeline::page(&source, animation).map(String::into_bytes)
                }
                Out::Still(format) => match flags.at {
                    Some(at) => diagram_pipeline::reveal_at(&renderer, &source, format, at),
                    None => diagram_pipeline::still(&renderer, &source, format),
                },
                Out::Motion(format) => diagram_pipeline::reveal(&renderer, &source, format, flags.fps),
            }
            .map_err(|error| fail(path, error))?;
            std::fs::write(out, bytes).map_err(|error| format!("{out}: {error}"))
        }
        "diff" => {
            let [before, after] = flags.files.as_slice() else {
                return Err(USAGE.to_string());
            };
            let out = flags.out.as_deref().ok_or(USAGE)?;
            let (b, a) = (read(before)?, read(after)?);
            let bytes = match out_kind(out, flags.reveal)? {
                Out::Html => {
                    let animation = if flags.reveal {
                        Animation::Reveal
                    } else {
                        Animation::None
                    };
                    diagram_pipeline::diff_page(&b, &a, animation == Animation::Reveal).map(String::into_bytes)
                }
                Out::Still(format) => match flags.at {
                    Some(at) => diagram_pipeline::diff_at(&renderer, &b, &a, format, at),
                    None => diagram_pipeline::diff_still(&renderer, &b, &a, format),
                },
                Out::Motion(format) => diagram_pipeline::diff_animation(&renderer, &b, &a, format, flags.fps),
            }
            .map_err(|error| fail(&format!("{before} → {after}"), error))?;
            std::fs::write(out, bytes).map_err(|error| format!("{out}: {error}"))
        }
        _ => Err(format!("no command {command}\n\n{USAGE}")),
    }
}
