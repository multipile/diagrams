//! The look every diagram shares: its colours, adapted from the drawing
//! rules of Diagram Design, and the timing of its animations.

use crate::document::Rgba;

pub const PAPER: Rgba = Rgba::hex(0xf5f5f5);
pub const INK: Rgba = Rgba::hex(0x2d3142);
pub const MUTED: Rgba = Rgba::hex(0x4f5d75);
pub const RULE: Rgba = Rgba::hex(0xd5d8de);
pub const WHITE: Rgba = Rgba::hex(0xffffff);
pub const STORE: Rgba = Rgba::hex(0xeceef2);
pub const ACCENT: Rgba = Rgba::hex(0xbf4520);
/// The accent, as a box's fill.
pub const ACCENT_TINT: Rgba = Rgba::hex(0xfbeee9);
pub const LINK: Rgba = Rgba::hex(0x2e5aa8);
pub const ADDED: Rgba = Rgba::hex(0x2f7d4f);
pub const REMOVED: Rgba = Rgba::hex(0xb3261e);
pub const CHANGED: Rgba = Rgba::hex(0xa86a12);

/// How long one step of a reveal waits after the step before it.
pub const STEP_MS: u32 = 240;
/// How long an element takes to appear.
pub const FADE_MS: u32 = 400;
/// How long the finished picture is held before a reveal ends.
pub const HOLD_MS: u32 = 1600;

/// When each part of a diff's animation starts, and how long it takes.
pub mod diff_timing {
    /// The picture as it was, before anything changes.
    pub const BEFORE_MS: u32 = 900;
    pub const REMOVE_MS: u32 = 600;
    pub const MOVE_MS: u32 = 800;
    pub const ADD_MS: u32 = 600;
    pub const MARK_MS: u32 = 300;
    pub const HOLD_MS: u32 = 1800;
    pub const REMOVE_AT: u32 = BEFORE_MS;
    pub const MOVE_AT: u32 = REMOVE_AT + REMOVE_MS;
    pub const ADD_AT: u32 = MOVE_AT + MOVE_MS;
    pub const MARK_AT: u32 = ADD_AT + ADD_MS;
    pub const TOTAL_MS: u32 = MARK_AT + MARK_MS + HOLD_MS;
}
