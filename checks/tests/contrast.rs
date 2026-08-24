//! Every `@contrast` annotation in the token layer, verified.
//!
//! This is the check the /accessibility page's contrast claim rests on. If it
//! passes, every pairing the stylesheet declares clears its level in the scheme
//! it was declared in. If a pairing is not annotated it is not checked, so the
//! second test here is the one that keeps the first honest: it fails when a
//! color role exists without a pairing that exercises it.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use checks::{Level, Rgb, contrast, pairings, tokens_for_scheme};

fn tokens_css() -> String {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../static/css/tokens.css");
    std::fs::read_to_string(path).expect("static/css/tokens.css is readable")
}

#[test]
fn every_declared_pairing_meets_its_level() {
    let css = tokens_css();
    let annotations = pairings(&css);
    assert!(
        annotations.len() >= 16,
        "expected the token layer to declare its pairings; found {}",
        annotations.len()
    );

    let light = tokens_for_scheme(&css, "light");
    let dark = tokens_for_scheme(&css, "dark");

    let mut failures = Vec::new();

    for p in &annotations {
        let table = if p.scheme == "dark" { &dark } else { &light };

        let Some(&fg) = table.get(&p.foreground) else {
            failures.push(format!(
                "tokens.css:{}: {} is not defined in the {} scheme",
                p.line, p.foreground, p.scheme
            ));
            continue;
        };
        let Some(&bg) = table.get(&p.background) else {
            failures.push(format!(
                "tokens.css:{}: {} is not defined in the {} scheme",
                p.line, p.background, p.scheme
            ));
            continue;
        };

        let ratio = contrast(fg, bg);
        if ratio < p.level.minimum() {
            failures.push(format!(
                "tokens.css:{}: {} on {} ({}) is {:.2}:1, below {} ({:.1}:1)",
                p.line,
                p.foreground,
                p.background,
                p.scheme,
                ratio,
                p.level.name(),
                p.level.minimum(),
            ));
        }
    }

    assert!(failures.is_empty(), "\n{}\n", failures.join("\n"));
}

#[test]
fn every_text_role_is_checked_against_every_surface() {
    // The annotations are hand-written, so the failure mode is forgetting one.
    // A text or accent role that is never paired with a surface is a hole in the
    // check above, and it is silent unless something looks for it.
    let css = tokens_css();
    let annotations = pairings(&css);

    let text_roles = ["--text", "--text-muted", "--link"];
    let surfaces = ["--surface", "--surface-sunken"];
    let mut missing = Vec::new();

    for scheme in ["light", "dark"] {
        for role in text_roles {
            for surface in surfaces {
                let found = annotations.iter().any(|p| {
                    p.scheme == scheme
                        && p.foreground == role
                        && p.background == surface
                        && p.level == Level::Aa
                });
                if !found {
                    missing.push(format!(
                        "{scheme}: {role} on {surface} is not checked at AA"
                    ));
                }
            }
        }
        // A control boundary and a focus ring are components, not text.
        for role in ["--border-control", "--focus"] {
            for surface in surfaces {
                let found = annotations.iter().any(|p| {
                    p.scheme == scheme
                        && p.foreground == role
                        && p.background == surface
                        && p.level == Level::Ui
                });
                if !found {
                    missing.push(format!(
                        "{scheme}: {role} on {surface} is not checked at UI"
                    ));
                }
            }
        }
    }

    assert!(missing.is_empty(), "\n{}\n", missing.join("\n"));
}

#[test]
fn the_button_fill_carries_readable_text_in_both_schemes() {
    let css = tokens_css();
    for scheme in ["light", "dark"] {
        let t = tokens_for_scheme(&css, scheme);
        let accent = t.get("--accent").copied().expect("--accent is defined");
        let on = t
            .get("--on-accent")
            .copied()
            .expect("--on-accent is defined");
        let ratio = contrast(on, accent);
        assert!(
            ratio >= 4.5,
            "{scheme}: --on-accent on --accent is {ratio:.2}:1, below AA"
        );
    }
}

#[test]
fn the_dark_scheme_actually_overrides_the_light_one() {
    // A dark block that redefines nothing is the failure that produces a white
    // page under a dark system setting, and it looks fine in the source.
    let css = tokens_css();
    let light = tokens_for_scheme(&css, "light");
    let dark = tokens_for_scheme(&css, "dark");

    for role in [
        "--surface",
        "--surface-sunken",
        "--text",
        "--text-muted",
        "--link",
        "--accent",
        "--on-accent",
        "--border",
        "--border-control",
        "--focus",
    ] {
        let l = light.get(role).expect("defined in light");
        let d = dark.get(role).expect("defined in dark");
        assert_ne!(l, d, "{role} is the same color in both schemes");
    }
}

#[test]
fn the_brand_palette_is_the_one_in_the_brand_sheet() {
    // The palette is not this repository's to change. If BRAND.md moves, this
    // fails and somebody decides deliberately rather than by drift.
    let css = tokens_css();
    let light = tokens_for_scheme(&css, "light");
    for (role, expected) in [
        ("--brand-ink", "#221B16"),
        ("--brand-clay", "#B14B2E"),
        ("--brand-clay-light", "#E08963"),
        ("--brand-peach", "#FFD9A8"),
        ("--brand-cream", "#FAF5EF"),
    ] {
        assert_eq!(
            light.get(role).copied(),
            Rgb::parse(expected),
            "{role} does not match assets/brand/BRAND.md"
        );
    }
}

#[test]
fn contrast_is_symmetric_and_bounded() {
    let white = Rgb::parse("#FFFFFF").unwrap();
    let black = Rgb::parse("#000000").unwrap();
    assert!((contrast(white, black) - 21.0).abs() < 0.01);
    assert!((contrast(black, white) - 21.0).abs() < 0.01);
    assert!((contrast(white, white) - 1.0).abs() < 0.01);
}

#[test]
fn shorthand_hex_parses_the_same_as_longhand() {
    assert_eq!(Rgb::parse("#fff"), Rgb::parse("#FFFFFF"));
    assert_eq!(Rgb::parse("#B14B2E"), Rgb::parse("#b14b2e"));
    assert_eq!(Rgb::parse("var(--text)"), None);
    assert_eq!(Rgb::parse("rebeccapurple"), None);
}
