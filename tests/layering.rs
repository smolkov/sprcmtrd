//! Keeps the module layers apart: the commands (CLI) and the TUI share the
//! library modules but never use each other, and the library modules don't
//! use the layers above them. Checked from the `crate::…` paths in the sources.

use std::collections::BTreeSet;
use std::path::Path;

/// Top-level modules each layer may use (besides itself).
const ALLOWED: &[(&str, &[&str])] = &[
    ("comtrade", &["error"]),
    ("analysis", &["comtrade"]),
    ("format", &["comtrade"]),
    ("term", &["args"]),
    (
        "command",
        &["analysis", "args", "complete", "comtrade", "context", "format", "term"],
    ),
    (
        "tui",
        &["analysis", "args", "comtrade", "config", "context", "format", "term"],
    ),
];

/// The `crate::<module>` names used in the `.rs` files below `dir`.
fn crate_modules(dir: &Path, out: &mut BTreeSet<(String, String)>) {
    for entry in std::fs::read_dir(dir).unwrap().flatten() {
        let path = entry.path();
        if path.is_dir() {
            crate_modules(&path, out);
            continue;
        }
        if path.extension().is_none_or(|e| e != "rs") {
            continue;
        }
        let text = std::fs::read_to_string(&path).unwrap();
        for (i, _) in text.match_indices("crate::") {
            let name: String = text[i + 7..]
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                .collect();
            out.insert((name, path.display().to_string()));
        }
    }
}

#[test]
fn layers_only_use_what_they_may() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut wrong = Vec::new();
    for (layer, allowed) in ALLOWED {
        let mut used = BTreeSet::new();
        crate_modules(&src.join(layer), &mut used);
        for (module, file) in used {
            if module != *layer && !allowed.contains(&module.as_str()) {
                wrong.push(format!("{file} uses crate::{module}"));
            }
        }
    }
    assert!(wrong.is_empty(), "layering broken:\n{}", wrong.join("\n"));
}

#[test]
fn the_check_finds_a_wrong_use() {
    // Guards the test above: it must see the paths it checks.
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut used = BTreeSet::new();
    crate_modules(&src.join("command"), &mut used);
    assert!(used.iter().any(|(m, _)| m == "format"));
    assert!(!used.iter().any(|(m, _)| m == "tui"));
}
