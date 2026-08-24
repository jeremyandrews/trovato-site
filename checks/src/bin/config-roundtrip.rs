//! Compare a config directory against an export of the site it built.
//!
//!     config-roundtrip <declared-dir> <exported-dir>
//!
//! The check behind "the site definition really lives in git": every key the
//! repository declares must come back from `trovato config export` with the same
//! value. An export carries more than the site declares — stages, roles and
//! content types the kernel and its plugins create — so this is a one-way
//! comparison, not a diff. What it catches is a declaration that imported into
//! nothing, or that the database quietly changed on the way in.
#![allow(clippy::print_stderr, clippy::print_stdout)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Keys the database assigns rather than the file, and which an export therefore
/// reproduces from the row rather than from what was imported.
///
/// `created` and `changed` are on the list because `save_item`'s `ON CONFLICT`
/// clause does not update them: re-importing a file with a corrected date is a
/// no-op on an existing row. That is worth knowing and is recorded in the
/// ledger, but it is not what this check is for.
const ASSIGNED_BY_DATABASE: [&str; 2] = ["created", "changed"];

fn main() -> std::process::ExitCode {
    let args: Vec<String> = std::env::args().collect();
    let (Some(declared), Some(exported)) = (args.get(1), args.get(2)) else {
        eprintln!("usage: config-roundtrip <declared-dir> <exported-dir>");
        return std::process::ExitCode::from(2);
    };

    let declared_files = match yaml_files(Path::new(declared)) {
        Ok(files) => files,
        Err(e) => {
            eprintln!("cannot read {declared}: {e}");
            return std::process::ExitCode::from(2);
        }
    };

    if declared_files.is_empty() {
        eprintln!("{declared} declares no config at all, which cannot be right");
        return std::process::ExitCode::from(2);
    }

    let mut problems: Vec<String> = Vec::new();

    for path in &declared_files {
        let name = path.file_name().unwrap_or_default().to_string_lossy();
        let counterpart = Path::new(exported).join(name.as_ref());

        let Ok(declared_text) = std::fs::read_to_string(path) else {
            problems.push(format!("{name}: unreadable"));
            continue;
        };
        let Ok(exported_text) = std::fs::read_to_string(&counterpart) else {
            problems.push(format!(
                "{name}: declared but absent from the export — it imported into nothing"
            ));
            continue;
        };

        let declared_value: serde_yml::Value = match serde_yml::from_str(&declared_text) {
            Ok(v) => v,
            Err(e) => {
                problems.push(format!("{name}: does not parse: {e}"));
                continue;
            }
        };
        let exported_value: serde_yml::Value = match serde_yml::from_str(&exported_text) {
            Ok(v) => v,
            Err(e) => {
                problems.push(format!("{name}: export does not parse: {e}"));
                continue;
            }
        };

        compare(&name, &declared_value, &exported_value, &mut problems);
    }

    println!("{} declared entities compared", declared_files.len());

    if problems.is_empty() {
        println!("every declared key round-trips unchanged");
        return std::process::ExitCode::SUCCESS;
    }

    eprintln!("\n{} difference(s):", problems.len());
    for p in &problems {
        eprintln!("  {p}");
    }
    std::process::ExitCode::FAILURE
}

/// Every top-level key the declared file sets must appear in the export with the
/// same value.
fn compare(
    name: &str,
    declared: &serde_yml::Value,
    exported: &serde_yml::Value,
    problems: &mut Vec<String>,
) {
    let (Some(a), Some(b)) = (declared.as_mapping(), exported.as_mapping()) else {
        if declared != exported {
            problems.push(format!("{name}: not a mapping, and the two differ"));
        }
        return;
    };

    for (key, declared_value) in a {
        let Some(key_name) = key.as_str() else {
            continue;
        };
        if ASSIGNED_BY_DATABASE.contains(&key_name) {
            continue;
        }

        match b.get(key) {
            None => problems.push(format!("{name}: '{key_name}' absent from the export")),
            Some(exported_value) if !equivalent(declared_value, exported_value) => {
                problems.push(format!(
                    "{name}: '{key_name}'\n      declared: {}\n      exported: {}",
                    render(declared_value),
                    render(exported_value)
                ));
            }
            Some(_) => {}
        }
    }
}

/// Whether two values mean the same thing.
///
/// Two shapes differ without disagreeing. A scalar the file wrote as a string
/// and the export wrote unquoted is the same value to a YAML reader but not to
/// `PartialEq` when one side parsed as a number. And a mapping's key order is
/// not part of its meaning, which `serde_yml::Mapping` preserves and compares.
fn equivalent(a: &serde_yml::Value, b: &serde_yml::Value) -> bool {
    match (a, b) {
        (serde_yml::Value::Mapping(x), serde_yml::Value::Mapping(y)) => {
            let sorted = |m: &serde_yml::Mapping| -> BTreeMap<String, serde_yml::Value> {
                m.iter()
                    .filter_map(|(k, v)| Some((k.as_str()?.to_string(), v.clone())))
                    .collect()
            };
            let (x, y) = (sorted(x), sorted(y));
            x.len() == y.len()
                && x.iter()
                    .all(|(k, v)| y.get(k).is_some_and(|w| equivalent(v, w)))
        }
        (serde_yml::Value::Sequence(x), serde_yml::Value::Sequence(y)) => {
            x.len() == y.len() && x.iter().zip(y).all(|(v, w)| equivalent(v, w))
        }
        _ => a == b || scalar_text(a) == scalar_text(b),
    }
}

/// A scalar as the text a YAML reader would see, or `None` for anything else.
fn scalar_text(value: &serde_yml::Value) -> Option<String> {
    match value {
        serde_yml::Value::String(s) => Some(s.trim().to_string()),
        serde_yml::Value::Number(n) => Some(n.to_string()),
        serde_yml::Value::Bool(b) => Some(b.to_string()),
        _ => None,
    }
}

fn render(value: &serde_yml::Value) -> String {
    let text = serde_yml::to_string(value).unwrap_or_else(|_| "<unrenderable>".into());
    let text = text.trim().replace('\n', "\n                ");
    if text.len() > 400 {
        format!("{}…", &text[..400])
    } else {
        text
    }
}

/// Every `.yml` file directly in a directory, sorted.
fn yaml_files(dir: &Path) -> std::io::Result<Vec<PathBuf>> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "yml"))
        .collect();
    files.sort();
    Ok(files)
}
