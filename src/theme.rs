use ratatui::style::Color;

use crate::error::ThothError;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Theme {
    pub selection_bg: Color,
    pub selection_fg: Color,
    pub accent: Color,
    pub dim: Color,
    pub border: Color,
    pub ok: Color,
    pub fail: Color,
    pub project: Color,
    pub command: Color,
    pub header: Color,
    pub controls: Color,
    pub directory: Color,
    pub tags: Color,
}

impl Default for Theme {
    fn default() -> Self {
        default_theme()
    }
}

const fn rgb(r: u8, g: u8, b: u8) -> Color {
    Color::Rgb(r, g, b)
}

fn default_theme() -> Theme {
    Theme {
        selection_bg: Color::Indexed(147),
        selection_fg: Color::Black,
        accent: Color::Cyan,
        dim: Color::DarkGray,
        border: Color::DarkGray,
        ok: Color::Green,
        fail: Color::Red,
        project: Color::Blue,
        command: Color::Reset,
        header: Color::Green,
        controls: Color::Green,
        directory: Color::DarkGray,
        tags: Color::Reset,
    }
}

fn ember_theme() -> Theme {
    Theme {
        selection_bg: rgb(0x66, 0x5c, 0x54),
        selection_fg: rgb(0xfb, 0xf1, 0xc7),
        accent: rgb(0xfa, 0xbd, 0x2f),
        dim: rgb(0x92, 0x83, 0x74),
        border: rgb(0x92, 0x83, 0x74),
        ok: rgb(0xb8, 0xbb, 0x26),
        fail: rgb(0xfb, 0x49, 0x34),
        project: rgb(0x83, 0xa5, 0x98),
        command: rgb(0xeb, 0xdb, 0xb2),
        header: rgb(0xfe, 0x80, 0x19),
        controls: rgb(0xfe, 0x80, 0x19),
        directory: rgb(0x92, 0x83, 0x74),
        tags: rgb(0xeb, 0xdb, 0xb2),
    }
}

fn frost_theme() -> Theme {
    Theme {
        selection_bg: rgb(0x43, 0x4c, 0x5e),
        selection_fg: rgb(0xec, 0xef, 0xf4),
        accent: rgb(0x88, 0xc0, 0xd0),
        dim: rgb(0x4c, 0x56, 0x6a),
        border: rgb(0x4c, 0x56, 0x6a),
        ok: rgb(0xa3, 0xbe, 0x8c),
        fail: rgb(0xbf, 0x61, 0x6a),
        project: rgb(0x81, 0xa1, 0xc1),
        command: rgb(0xd8, 0xde, 0xe9),
        header: rgb(0x8f, 0xbc, 0xbb),
        controls: rgb(0x88, 0xc0, 0xd0),
        directory: rgb(0x4c, 0x56, 0x6a),
        tags: rgb(0xd8, 0xde, 0xe9),
    }
}

fn latte_theme() -> Theme {
    Theme {
        selection_bg: rgb(0xcc, 0xd0, 0xda),
        selection_fg: rgb(0x4c, 0x4f, 0x69),
        accent: rgb(0x17, 0x92, 0x99),
        dim: rgb(0x9c, 0xa0, 0xb0),
        border: rgb(0x9c, 0xa0, 0xb0),
        ok: rgb(0x40, 0xa0, 0x2b),
        fail: rgb(0xd2, 0x0f, 0x39),
        project: rgb(0x1e, 0x66, 0xf5),
        command: rgb(0x4c, 0x4f, 0x69),
        header: rgb(0x88, 0x39, 0xef),
        controls: rgb(0x88, 0x39, 0xef),
        directory: rgb(0x9c, 0xa0, 0xb0),
        tags: rgb(0x4c, 0x4f, 0x69),
    }
}

fn frappe_theme() -> Theme {
    Theme {
        selection_bg: rgb(0x41, 0x45, 0x59),
        selection_fg: rgb(0xc6, 0xd0, 0xf5),
        accent: rgb(0x81, 0xc8, 0xbe),
        dim: rgb(0x73, 0x79, 0x94),
        border: rgb(0x73, 0x79, 0x94),
        ok: rgb(0xa6, 0xd1, 0x89),
        fail: rgb(0xe7, 0x82, 0x84),
        project: rgb(0x8c, 0xaa, 0xee),
        command: rgb(0xc6, 0xd0, 0xf5),
        header: rgb(0xca, 0x9e, 0xe6),
        controls: rgb(0xca, 0x9e, 0xe6),
        directory: rgb(0x73, 0x79, 0x94),
        tags: rgb(0xc6, 0xd0, 0xf5),
    }
}

fn macchiato_theme() -> Theme {
    Theme {
        selection_bg: rgb(0x36, 0x3a, 0x4f),
        selection_fg: rgb(0xca, 0xd3, 0xf5),
        accent: rgb(0x8b, 0xd5, 0xca),
        dim: rgb(0x6e, 0x73, 0x8d),
        border: rgb(0x6e, 0x73, 0x8d),
        ok: rgb(0xa6, 0xda, 0x95),
        fail: rgb(0xed, 0x87, 0x96),
        project: rgb(0x8a, 0xad, 0xf4),
        command: rgb(0xca, 0xd3, 0xf5),
        header: rgb(0xc6, 0xa0, 0xf6),
        controls: rgb(0xc6, 0xa0, 0xf6),
        directory: rgb(0x6e, 0x73, 0x8d),
        tags: rgb(0xca, 0xd3, 0xf5),
    }
}

fn mocha_theme() -> Theme {
    Theme {
        selection_bg: rgb(0x31, 0x32, 0x44),
        selection_fg: rgb(0xcd, 0xd6, 0xf4),
        accent: rgb(0x94, 0xe2, 0xd5),
        dim: rgb(0x6c, 0x70, 0x86),
        border: rgb(0x6c, 0x70, 0x86),
        ok: rgb(0xa6, 0xe3, 0xa1),
        fail: rgb(0xf3, 0x8b, 0xa8),
        project: rgb(0x89, 0xb4, 0xfa),
        command: rgb(0xcd, 0xd6, 0xf4),
        header: rgb(0xcb, 0xa6, 0xf7),
        controls: rgb(0xcb, 0xa6, 0xf7),
        directory: rgb(0x6c, 0x70, 0x86),
        tags: rgb(0xcd, 0xd6, 0xf4),
    }
}

pub fn builtin_names() -> &'static [&'static str] {
    &[
        "default",
        "ember",
        "frost",
        "latte",
        "frappe",
        "macchiato",
        "mocha",
    ]
}

pub fn builtin(name: &str) -> Option<Theme> {
    match name.to_ascii_lowercase().as_str() {
        "default" => Some(default_theme()),
        "ember" => Some(ember_theme()),
        "frost" => Some(frost_theme()),
        "latte" => Some(latte_theme()),
        "frappe" => Some(frappe_theme()),
        "macchiato" => Some(macchiato_theme()),
        "mocha" => Some(mocha_theme()),
        _ => None,
    }
}

pub fn parse_color(s: &str) -> Result<Color, ThothError> {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return Err(ThothError::Config(format!("invalid color: {s}")));
    }

    if let Some(hex) = trimmed.strip_prefix('#') {
        return parse_hex(hex, s);
    }

    if let Ok(n) = trimmed.parse::<u16>() {
        if n <= 255 {
            return Ok(Color::Indexed(n as u8));
        }
        return Err(ThothError::Config(format!("invalid color: {s}")));
    }

    match trimmed.to_ascii_lowercase().as_str() {
        "reset" | "default" => Ok(Color::Reset),
        "black" => Ok(Color::Black),
        "red" => Ok(Color::Red),
        "green" => Ok(Color::Green),
        "yellow" => Ok(Color::Yellow),
        "blue" => Ok(Color::Blue),
        "magenta" => Ok(Color::Magenta),
        "cyan" => Ok(Color::Cyan),
        "white" => Ok(Color::White),
        "gray" | "grey" => Ok(Color::Gray),
        "darkgray" | "darkgrey" => Ok(Color::DarkGray),
        "brightblack" => Ok(Color::DarkGray),
        "brightred" => Ok(Color::LightRed),
        "brightgreen" => Ok(Color::LightGreen),
        "brightyellow" => Ok(Color::LightYellow),
        "brightblue" => Ok(Color::LightBlue),
        "brightmagenta" => Ok(Color::LightMagenta),
        "brightcyan" => Ok(Color::LightCyan),
        "brightwhite" => Ok(Color::White),
        _ => Err(ThothError::Config(format!("invalid color: {s}"))),
    }
}

fn parse_hex(hex: &str, original: &str) -> Result<Color, ThothError> {
    match hex.len() {
        6 => {
            let r = u8::from_str_radix(&hex[0..2], 16)
                .map_err(|_| ThothError::Config(format!("invalid color: {original}")))?;
            let g = u8::from_str_radix(&hex[2..4], 16)
                .map_err(|_| ThothError::Config(format!("invalid color: {original}")))?;
            let b = u8::from_str_radix(&hex[4..6], 16)
                .map_err(|_| ThothError::Config(format!("invalid color: {original}")))?;
            Ok(Color::Rgb(r, g, b))
        }
        3 => {
            let r = expand_nibble(&hex[0..1], original)?;
            let g = expand_nibble(&hex[1..2], original)?;
            let b = expand_nibble(&hex[2..3], original)?;
            Ok(Color::Rgb(r, g, b))
        }
        _ => Err(ThothError::Config(format!("invalid color: {original}"))),
    }
}

fn expand_nibble(h: &str, original: &str) -> Result<u8, ThothError> {
    let n = u8::from_str_radix(h, 16)
        .map_err(|_| ThothError::Config(format!("invalid color: {original}")))?;
    Ok(n << 4 | n)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_hex6_color() {
        assert_eq!(parse_color("#ff8800").unwrap(), Color::Rgb(255, 136, 0));
    }

    #[test]
    fn parse_hex3_color() {
        assert_eq!(parse_color("#f80").unwrap(), Color::Rgb(0xff, 0x88, 0x00));
    }

    #[test]
    fn parse_indexed_147() {
        assert_eq!(parse_color("147").unwrap(), Color::Indexed(147));
    }

    #[test]
    fn parse_indexed_boundaries() {
        assert_eq!(parse_color("0").unwrap(), Color::Indexed(0));
        assert_eq!(parse_color("255").unwrap(), Color::Indexed(255));
    }

    #[test]
    fn parse_indexed_out_of_range() {
        assert!(parse_color("256").is_err());
    }

    #[test]
    fn parse_reset_and_default() {
        assert_eq!(parse_color("reset").unwrap(), Color::Reset);
        assert_eq!(parse_color("default").unwrap(), Color::Reset);
    }

    #[test]
    fn parse_named_colors() {
        assert_eq!(parse_color("red").unwrap(), Color::Red);
        assert_eq!(parse_color("green").unwrap(), Color::Green);
        assert_eq!(parse_color("blue").unwrap(), Color::Blue);
        assert_eq!(parse_color("cyan").unwrap(), Color::Cyan);
        assert_eq!(parse_color("darkgray").unwrap(), Color::DarkGray);
        assert_eq!(parse_color("darkgrey").unwrap(), Color::DarkGray);
    }

    #[test]
    fn parse_bright_variants() {
        assert_eq!(parse_color("brightblue").unwrap(), Color::LightBlue);
        assert_eq!(parse_color("brightred").unwrap(), Color::LightRed);
        assert_eq!(parse_color("brightblack").unwrap(), Color::DarkGray);
    }

    #[test]
    fn parse_uppercase_name() {
        assert_eq!(parse_color("RED").unwrap(), Color::Red);
    }

    #[test]
    fn parse_invalid_name() {
        assert!(parse_color("nope").is_err());
    }

    #[test]
    fn parse_invalid_hex() {
        assert!(parse_color("#xyz").is_err());
        assert!(parse_color("#12").is_err());
    }

    #[test]
    fn parse_empty_string() {
        assert!(parse_color("").is_err());
        assert!(parse_color("   ").is_err());
    }

    #[test]
    fn builtin_mocha_values() {
        let t = builtin("mocha").unwrap();
        assert_eq!(t.selection_bg, Color::Rgb(0x31, 0x32, 0x44));
        assert_eq!(t.ok, Color::Rgb(0xa6, 0xe3, 0xa1));
    }

    #[test]
    fn builtin_mocha_case_insensitive() {
        assert!(builtin("MOCHA").is_some());
    }

    #[test]
    fn builtin_unknown_is_none() {
        assert!(builtin("nope").is_none());
    }

    #[test]
    fn builtin_names_has_seven() {
        assert_eq!(builtin_names().len(), 7);
        assert!(builtin_names().contains(&"default"));
        assert!(builtin_names().contains(&"mocha"));
    }

    #[test]
    fn default_impl_matches_builtin() {
        assert_eq!(Theme::default(), builtin("default").unwrap());
    }

    #[test]
    fn default_theme_accent_and_selection() {
        let t = Theme::default();
        assert_eq!(t.accent, Color::Cyan);
        assert_eq!(t.selection_bg, Color::Indexed(147));
    }

    #[test]
    fn all_builtins_retrievable_and_distinct() {
        let themes: Vec<Theme> = builtin_names()
            .iter()
            .map(|n| builtin(n).unwrap())
            .collect();
        assert_eq!(themes.len(), 7);
        let mocha = builtin("mocha").unwrap();
        let latte = builtin("latte").unwrap();
        assert_ne!(mocha, latte);
    }
}
