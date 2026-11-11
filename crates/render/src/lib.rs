//! Renders a diagram's document with takumi, with no browser and nothing
//! parsed: to a still picture, or to an animated one.
//!
//! Each node of the document is built as takumi's node, with its style set
//! property by property. An animation is drawn frame by frame: for each
//! frame, the renderer works out where each track puts its nodes at that
//! moment, builds the nodes so, and has takumi draw them, the same way the
//! page's CSS moves them in a browser.
//!
//! Takumi loads no system fonts, so the renderer registers the fonts the
//! document is set in, which `diagram-document` carries.

use std::borrow::Cow;

use diagram_document::document::{self as doc, Align, Content, Document, Family, LineStyle, Place, Rgba};
use takumi::prelude::{
    AlignItems, Angle, AnimatedGifOptions, AnimatedPngOptions, AnimatedWebpOptions, AnimationFrame, Bitmap,
    BorderStyle, BoxSizing, Color, ColorInput, Display, FlexDirection, FontFamily, FontResource, FontSize, FontWeight,
    Fonts, JustifyContent, Length, LineHeight, LineWidth, Node, OutputFormat, Overflow, PercentageNumber, Position,
    RenderOptions, SpacePair, Style, StyleDeclaration, TextAlign, TextDecorationLines, TextWrapMode, Viewport,
};
use takumi::{render, write_animated_gif, write_animated_png, write_animated_webp, write_image};

/// The format of a still picture.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StillFormat {
    Png,
    /// Lossless WebP.
    WebP,
}

/// The format of an animated picture.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MotionFormat {
    Gif,
    WebP,
    /// Animated PNG.
    Apng,
}

impl MotionFormat {
    /// The highest frame rate the format plays as written: browsers hold a
    /// frame of 10 ms or less for 100 ms, and a GIF counts in steps of 10.
    pub fn max_fps(self) -> u32 {
        match self {
            MotionFormat::Gif => 50,
            MotionFormat::WebP | MotionFormat::Apng => 90,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RenderError {
    /// A document that does not move has no animation.
    Still,
    /// Takumi could not draw or encode the document.
    Takumi(String),
}

impl std::fmt::Display for RenderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RenderError::Still => write!(f, "the diagram does not move, so it has no animation: render it still"),
            RenderError::Takumi(why) => write!(f, "takumi could not render the diagram: {why}"),
        }
    }
}

impl std::error::Error for RenderError {}

fn takumi_error(error: impl std::fmt::Display) -> RenderError {
    RenderError::Takumi(error.to_string())
}

/// Renders documents, with the fonts registered once.
pub struct Renderer {
    fonts: Fonts,
    scale: f32,
}

impl Default for Renderer {
    fn default() -> Renderer {
        Renderer::new()
    }
}

impl Renderer {
    /// A renderer at twice the document's size, for a sharp picture on a
    /// screen of any density.
    pub fn new() -> Renderer {
        let mut fonts = Fonts::default();
        for font in diagram_document::fonts::FONTS {
            fonts
                .register(FontResource::new(font.woff2))
                .expect("the fonts diagram-document carries are valid WOFF2");
        }
        Renderer { fonts, scale: 2.0 }
    }

    /// The same renderer, drawing `scale` pixels for each pixel of the
    /// document.
    pub fn with_scale(mut self, scale: f32) -> Renderer {
        self.scale = scale;
        self
    }

    pub fn scale(&self) -> f32 {
        self.scale
    }

    /// The picture's size in pixels: the document's, times the scale.
    /// Takumi sizes the viewport in pixels, and draws each pixel of the
    /// document as `device_pixel_ratio` of them.
    fn viewport(&self, document: &Document) -> Viewport {
        let size = |px: i32| (px as f32 * self.scale).round() as u32;
        let mut viewport = Viewport::new((size(document.width), size(document.height)));
        viewport.device_pixel_ratio = self.scale;
        viewport
    }

    fn frame(&self, document: &Document, time_ms: u32) -> Result<Bitmap, RenderError> {
        render(
            RenderOptions::builder()
                .viewport(self.viewport(document))
                .node(tree(document, time_ms))
                .fonts(&self.fonts)
                .build(),
        )
        .map_err(takumi_error)
    }

    /// A still picture of the document: at `at_ms` into its animation, or
    /// at its end.
    pub fn still(&self, document: &Document, format: StillFormat, at_ms: Option<u32>) -> Result<Vec<u8>, RenderError> {
        let image = self.frame(document, at_ms.unwrap_or(document.duration_ms))?;
        let mut out = Vec::new();
        let format = match format {
            StillFormat::Png => OutputFormat::Png,
            StillFormat::WebP => OutputFormat::WebPLossless,
        };
        write_image(&image, &mut out, format).map_err(takumi_error)?;
        Ok(out)
    }

    /// The document's animation, from its start to its end, at `fps` frames
    /// a second, or the most the format plays as written when `fps` is more.
    pub fn animation(&self, document: &Document, format: MotionFormat, fps: u32) -> Result<Vec<u8>, RenderError> {
        if document.duration_ms == 0 {
            return Err(RenderError::Still);
        }
        let fps = fps.clamp(1, format.max_fps());
        let count = (document.duration_ms * fps).div_ceil(1000);
        let mut frames = Vec::with_capacity(count as usize);
        for index in 0..count {
            let start = index * 1000 / fps;
            let end = ((index + 1) * 1000 / fps).min(document.duration_ms);
            // The last frame shows the end, and holds.
            let at = if index + 1 == count {
                document.duration_ms
            } else {
                start
            };
            frames.push(AnimationFrame::new(self.frame(document, at)?, end - start));
        }
        let mut out = Vec::new();
        match format {
            MotionFormat::Gif => write_animated_gif(Cow::Owned(frames), &mut out, AnimatedGifOptions::default()),
            MotionFormat::WebP => write_animated_webp(Cow::Owned(frames), &mut out, AnimatedWebpOptions::default()),
            MotionFormat::Apng => write_animated_png(&frames, &mut out, AnimatedPngOptions::default()),
        }
        .map_err(takumi_error)?;
        Ok(out)
    }
}

/// The document as takumi's nodes at `time_ms`: the diagram's frame, and
/// every node in it, each where its track puts it then.
pub fn tree(document: &Document, time_ms: u32) -> Node {
    let children: Vec<Node> = document
        .nodes
        .iter()
        .map(|node| build(document, node, time_ms))
        .collect();
    Node::container(children).with_style(
        Style::default()
            .position(Position::Relative)
            .with(StyleDeclaration::width(px(document.width as f32)))
            .with(StyleDeclaration::height(px(document.height as f32)))
            .overflow_x(Overflow::Hidden)
            .overflow_y(Overflow::Hidden)
            .background_color(color(diagram_document::theme::PAPER)),
    )
}

fn px(value: f32) -> Length {
    Length::Px(value)
}

fn color(c: Rgba) -> ColorInput {
    ColorInput::from(Color([c.0, c.1, c.2, c.3]))
}

fn align_items(align: Align) -> AlignItems {
    match align {
        Align::Start => AlignItems::FlexStart,
        Align::Center => AlignItems::Center,
        Align::End => AlignItems::FlexEnd,
    }
}

fn justify(align: Align) -> JustifyContent {
    match align {
        Align::Start => JustifyContent::FlexStart,
        Align::Center => JustifyContent::Center,
        Align::End => JustifyContent::FlexEnd,
    }
}

fn edge(edge: Option<doc::Edge>) -> (LineWidth, BorderStyle, ColorInput) {
    match edge {
        Some(edge) => (
            LineWidth::from(px(edge.width)),
            match edge.style {
                LineStyle::Solid => BorderStyle::Solid,
                LineStyle::Dashed => BorderStyle::Dashed,
            },
            color(edge.color),
        ),
        None => (LineWidth::from(px(0.0)), BorderStyle::None, color(Rgba::TRANSPARENT)),
    }
}

fn build(document: &Document, node: &doc::Node, time_ms: u32) -> Node {
    let mut style = Style::default().box_sizing(BoxSizing::BorderBox);
    match node.place {
        Place::Absolute(r) => {
            style = style
                .position(Position::Absolute)
                .left(px(r.x as f32))
                .top(px(r.y as f32))
                .with(StyleDeclaration::width(px(r.w as f32)))
                .with(StyleDeclaration::height(px(r.h as f32)));
        }
        Place::Sized { w, h } => {
            style = style
                .position(Position::Relative)
                .min_width(px(w as f32))
                .min_height(px(h as f32))
                .with(StyleDeclaration::width(px(w as f32)))
                .with(StyleDeclaration::height(px(h as f32)));
        }
        Place::Flow => style = style.position(Position::Relative),
    }
    let s = &node.style;
    if let Some(flex) = s.flex {
        style = style
            .display(Display::Flex)
            .flex_direction(if flex.column {
                FlexDirection::Column
            } else {
                FlexDirection::Row
            })
            .align_items(align_items(flex.align))
            .justify_content(justify(flex.justify));
    }
    if let Some(background) = s.background {
        style = style.background_color(color(background));
    }
    let [top, right, bottom, left] = s.border;
    let (w, st, c) = edge(top);
    style = style.border_top_width(w).border_top_style(st).border_top_color(c);
    let (w, st, c) = edge(right);
    style = style.border_right_width(w).border_right_style(st).border_right_color(c);
    let (w, st, c) = edge(bottom);
    style = style
        .border_bottom_width(w)
        .border_bottom_style(st)
        .border_bottom_color(c);
    let (w, st, c) = edge(left);
    style = style.border_left_width(w).border_left_style(st).border_left_color(c);
    let [a, b, c, d] = s.radius;
    style = style
        .border_top_left_radius(SpacePair::from_single(px(a)))
        .border_top_right_radius(SpacePair::from_single(px(b)))
        .border_bottom_right_radius(SpacePair::from_single(px(c)))
        .border_bottom_left_radius(SpacePair::from_single(px(d)));
    let [pt, pr, pb, pl] = s.padding;
    style = style
        .padding_top(px(pt as f32))
        .padding_right(px(pr as f32))
        .padding_bottom(px(pb as f32))
        .padding_left(px(pl as f32));
    let pose = document.pose(node, time_ms);
    let opacity = s.opacity.unwrap_or(1.0) * pose.opacity;
    if opacity < 1.0 {
        style = style.opacity(PercentageNumber(opacity));
    }
    if pose.dx != 0.0 || pose.dy != 0.0 {
        style = style.translate(SpacePair::from_pair(px(pose.dx), px(pose.dy)));
    }
    if let Some(degrees) = s.rotate {
        style = style.rotate(Some(Angle::new(degrees)));
    }
    if s.clip {
        style = style.overflow_x(Overflow::Hidden).overflow_y(Overflow::Hidden);
    }
    match &node.content {
        Content::Text(text) => {
            let family = match text.family {
                Family::Sans => "Geist",
                Family::Mono => "Geist Mono",
            };
            let mut style = style
                .font_family(FontFamily::from_names([family.to_string()]))
                .font_size(FontSize::from(px(text.size)))
                .font_weight(FontWeight::from(text.weight as f32))
                .color(color(text.color))
                .line_height(LineHeight::from(px(text.line_height)))
                .letter_spacing(px(text.letter_spacing))
                .text_wrap_mode(TextWrapMode::NoWrap)
                .text_align(match text.align {
                    doc::TextAlign::Left => TextAlign::Left,
                    doc::TextAlign::Center => TextAlign::Center,
                });
            if text.strike {
                style = style.text_decoration_line(Some(TextDecorationLines::LINE_THROUGH));
            }
            Node::text(text.value.clone()).with_style(style)
        }
        Content::Children(children) => Node::container(
            children
                .iter()
                .map(|child| build(document, child, time_ms))
                .collect::<Vec<_>>(),
        )
        .with_style(style),
        Content::None => Node::container(Vec::<Node>::new()).with_style(style),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use diagram_document::document::{Ease, Node as DocNode, Pose, Style as DocStyle, Track};
    use diagram_document::scene::Rect;

    fn square() -> Document {
        Document {
            width: 120,
            height: 80,
            title: "A square".into(),
            desc: "A black square at the bottom right.".into(),
            duration_ms: 0,
            changes: None,
            nodes: vec![DocNode::absolute(
                Rect::new(110, 70, 10, 10),
                DocStyle::default().background(Rgba::hex(0x000000)),
            )],
            tracks: Vec::new(),
        }
    }

    fn decode(png: Vec<u8>) -> (u32, u32, Vec<u8>, usize) {
        let mut decoder = png::Decoder::new(std::io::Cursor::new(png));
        // Takumi writes a palette when the picture has few colours.
        decoder.set_transformations(png::Transformations::EXPAND);
        let mut reader = decoder.read_info().unwrap();
        let mut pixels = vec![0; reader.output_buffer_size().unwrap()];
        let info = reader.next_frame(&mut pixels).unwrap();
        let channels = info.line_size / info.width as usize;
        (info.width, info.height, pixels, channels)
    }

    #[test]
    fn a_still_document_has_no_animation() {
        let error = Renderer::new().animation(&square(), MotionFormat::Gif, 10).unwrap_err();
        assert_eq!(error, RenderError::Still);
    }

    #[test]
    fn a_picture_is_the_document_at_the_renderers_scale() {
        let png = Renderer::new()
            .with_scale(1.0)
            .still(&square(), StillFormat::Png, None)
            .unwrap();
        let (w, h, ..) = decode(png);
        assert_eq!((w, h), (120, 80));
    }

    #[test]
    fn a_scaled_picture_holds_the_whole_document() {
        let png = Renderer::new()
            .with_scale(2.0)
            .still(&square(), StillFormat::Png, None)
            .unwrap();
        let (w, h, pixels, channels) = decode(png);
        assert_eq!((w, h), (240, 160));
        let at = |x: usize, y: usize| pixels[(y * w as usize + x) * channels];
        assert!(at(230, 150) < 16, "the square is black");
        assert!(at(200, 120) > 200, "the paper around it is light");
    }

    #[test]
    fn a_frame_shows_each_node_where_its_track_puts_it_then() {
        let mut document = square();
        document.duration_ms = 1000;
        let hidden = Pose {
            opacity: 0.0,
            ..Pose::REST
        };
        document
            .tracks
            .push(Track::change(hidden, Pose::REST, 0, 1000, Ease::Linear));
        document.nodes[0].track = Some(0);
        let renderer = Renderer::new().with_scale(1.0);
        let at = |time| {
            let (w, _, pixels, channels) = decode(renderer.still(&document, StillFormat::Png, Some(time)).unwrap());
            pixels[(75 * w as usize + 115) * channels]
        };
        assert!(at(0) > 200, "hidden at the start");
        assert!(at(1000) < 16, "drawn at the end");
        let middle = at(500);
        assert!(middle > 60 && middle < 190, "half drawn halfway: {middle}");
    }
}
