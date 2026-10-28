pub mod code_block;
pub mod command_palette;
pub mod dialog;
pub mod fonts;
pub mod icons;
pub mod overlays;
pub mod primitives;
pub mod tabs;
pub mod theme;

/// Embedded Geist Mono WOFF2 bytes for the application's font-face stylesheet.
pub const GEIST_MONO_WOFF2: &[u8] = fonts::GEIST_MONO_WOFF2;
