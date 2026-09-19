//! Color palette, matching ghosttracer teal monochrome (reused from pdf-search)
use ratatui::style::Color;

pub const BG_DARKEST: Color = Color::Rgb(0, 8, 8);
pub const BG_DARK: Color = Color::Rgb(0, 24, 24);
pub const BG_MID: Color = Color::Rgb(0, 40, 40);
pub const BG_LIGHT: Color = Color::Rgb(0, 56, 56);

pub const TEXT: Color = Color::Rgb(208, 240, 240);
pub const TEXT_MUTED: Color = Color::Rgb(82, 163, 163);
pub const TEXT_DIM: Color = Color::Rgb(51, 115, 115);

pub const BRAND: Color = Color::Rgb(112, 240, 240);
pub const BRAND_DIM: Color = Color::Rgb(0, 176, 160);

pub const HIGHLIGHT: Color = Color::Rgb(240, 80, 240);
pub const SUCCESS: Color = Color::Rgb(0, 224, 160);
pub const ERROR: Color = Color::Rgb(255, 80, 80);
pub const INFO: Color = Color::Rgb(80, 160, 240);
pub const SEMANTIC: Color = Color::Rgb(208, 128, 240);

pub const SELECTION_BG: Color = Color::Rgb(0, 77, 77);
pub const WARNING: Color = Color::Rgb(240, 200, 80);
