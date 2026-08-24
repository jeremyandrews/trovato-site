//! Checks over the site's own files.
//!
//! Not a plugin and not shipped: this crate exists so that claims the site makes
//! about itself are verified by `cargo test` rather than by having been true once.
//! The contrast ratios on the accessibility page come from here.

/// A color as sRGB bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Rgb {
    /// Parse `#RGB` or `#RRGGBB`. Returns `None` for anything else, including a
    /// CSS color name or a `var(...)` that was not resolved first.
    pub fn parse(raw: &str) -> Option<Self> {
        let hex = raw.trim().strip_prefix('#')?;
        let byte = |s: &str| u8::from_str_radix(s, 16).ok();
        match hex.len() {
            3 => {
                let mut it = hex.chars();
                let dup = |c: char| byte(&format!("{c}{c}"));
                let r = dup(it.next()?)?;
                let g = dup(it.next()?)?;
                let b = dup(it.next()?)?;
                Some(Self { r, g, b })
            }
            6 => Some(Self {
                r: byte(&hex[0..2])?,
                g: byte(&hex[2..4])?,
                b: byte(&hex[4..6])?,
            }),
            _ => None,
        }
    }

    /// Relative luminance, WCAG 2.1 definition.
    pub fn luminance(self) -> f64 {
        fn channel(v: u8) -> f64 {
            let c = f64::from(v) / 255.0;
            if c <= 0.040_45 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            }
        }
        0.2126 * channel(self.r) + 0.7152 * channel(self.g) + 0.0722 * channel(self.b)
    }
}

/// The WCAG 2.1 contrast ratio between two colors, from 1.0 to 21.0.
pub fn contrast(a: Rgb, b: Rgb) -> f64 {
    let (la, lb) = (a.luminance(), b.luminance());
    let (hi, lo) = if la > lb { (la, lb) } else { (lb, la) };
    (hi + 0.05) / (lo + 0.05)
}

/// What a pairing has to clear.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    /// Body text: 4.5:1.
    Aa,
    /// Large text — 24px, or 18.66px bold: 3:1.
    AaLarge,
    /// A control boundary or a focus ring, WCAG 1.4.11: 3:1.
    Ui,
}

impl Level {
    pub fn parse(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_uppercase().as_str() {
            "AA" => Some(Self::Aa),
            "AA-LARGE" => Some(Self::AaLarge),
            "UI" => Some(Self::Ui),
            _ => None,
        }
    }

    pub fn minimum(self) -> f64 {
        match self {
            Self::Aa => 4.5,
            Self::AaLarge | Self::Ui => 3.0,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Aa => "AA",
            Self::AaLarge => "AA-LARGE",
            Self::Ui => "UI",
        }
    }
}

/// One `@contrast FG on BG LEVEL` annotation, with the scheme it was found in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pairing {
    pub foreground: String,
    pub background: String,
    pub level: Level,
    /// `"light"` or `"dark"`, from which block of the stylesheet it came.
    pub scheme: String,
    /// 1-indexed line, so a failure says where to look.
    pub line: usize,
}

/// Every custom property defined in one scheme, resolved to a color.
///
/// A stylesheet is read twice: once for the properties outside any media query
/// (light), once with the `prefers-color-scheme: dark` block's properties layered
/// over them (dark). That mirrors how a browser resolves them, and it is why a
/// dark override that is missing shows up as the light value failing its dark
/// annotation rather than as nothing at all.
pub fn tokens_for_scheme(css: &str, scheme: &str) -> std::collections::HashMap<String, Rgb> {
    let mut out = std::collections::HashMap::new();
    let mut depth_in_dark = None::<usize>;
    let mut depth = 0usize;

    for line in css.lines() {
        let trimmed = line.trim();

        if trimmed.contains("prefers-color-scheme: dark") {
            depth_in_dark = Some(depth);
        }

        let opens = trimmed.matches('{').count();
        let closes = trimmed.matches('}').count();
        depth += opens;

        let in_dark = depth_in_dark.is_some();
        let wanted = if scheme == "dark" { true } else { !in_dark };

        if wanted
            && let Some((name, value)) = declaration(trimmed)
            && let Some(rgb) = Rgb::parse(&value)
        {
            out.insert(name, rgb);
        }

        depth = depth.saturating_sub(closes);
        if let Some(d) = depth_in_dark
            && depth <= d
            && closes > 0
        {
            depth_in_dark = None;
        }
    }

    out
}

/// Split `--name: value;` into its two halves.
fn declaration(line: &str) -> Option<(String, String)> {
    let line = line.strip_prefix("--")?;
    let (name, rest) = line.split_once(':')?;
    let value = rest.trim().trim_end_matches(';').trim();
    Some((format!("--{}", name.trim()), value.to_string()))
}

/// Every `@contrast` annotation in a stylesheet.
///
/// An annotation belongs to the scheme of the block it sits in, which is what
/// makes one token name checkable twice with two different values.
pub fn pairings(css: &str) -> Vec<Pairing> {
    let mut out = Vec::new();
    let mut in_dark = false;
    let mut depth = 0usize;
    let mut dark_depth = 0usize;

    for (index, line) in css.lines().enumerate() {
        let trimmed = line.trim();

        if trimmed.contains("prefers-color-scheme: dark") {
            in_dark = true;
            dark_depth = depth;
        }

        if let Some(rest) = trimmed.strip_prefix("/* @contrast ") {
            let body = rest.trim_end_matches("*/").trim();
            let words: Vec<&str> = body.split_whitespace().collect();
            // FG on BG LEVEL
            if words.len() == 4
                && words[1] == "on"
                && let Some(level) = Level::parse(words[3])
            {
                out.push(Pairing {
                    foreground: words[0].to_string(),
                    background: words[2].to_string(),
                    level,
                    scheme: if in_dark { "dark" } else { "light" }.to_string(),
                    line: index + 1,
                });
            }
        }

        depth += trimmed.matches('{').count();
        let closes = trimmed.matches('}').count();
        depth = depth.saturating_sub(closes);
        if in_dark && closes > 0 && depth <= dark_depth {
            in_dark = false;
        }
    }

    out
}
