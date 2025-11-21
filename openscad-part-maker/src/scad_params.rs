use anyhow::Context;
use regex::Regex;
use std::{
    collections::{BTreeMap, HashSet},
    fs,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParamType {
    Number,
    Bool,
    String,
}

/// A parameter discovered from .scad defaults.
#[derive(Debug, Clone)]
pub struct ParamSpec {
    pub name: String,    // e.g. "COASTER_D"
    pub default: String, // RHS text as OpenSCAD syntax, e.g. "101.6" or "\"octagon\""
    pub ty: ParamType,
    #[allow(dead_code)]
    pub is_user_param: bool, // whether we consider it a real user-facing param
    pub comment: String,
    pub options: Vec<String>,
}

/// Template/specs discovered from the input .scad tree.
/// Cloneable and stored in AppState.
#[derive(Debug, Clone)]
pub struct ScadParamTemplate {
    pub specs: BTreeMap<String, ParamSpec>,
    pub defaults: BTreeMap<String, String>,
}

#[derive(Debug, Clone)]
pub struct ScadParams {
    pub specs: BTreeMap<String, ParamSpec>,
    pub values: BTreeMap<String, String>,
}

/// Parse common bool variants from HTML forms.
pub fn parse_bool(value: &str) -> Result<bool, ()> {
    match value.to_ascii_lowercase().as_str() {
        "1" | "true" | "on" | "yes" => Ok(true),
        "0" | "false" | "off" | "no" => Ok(false),
        _ => Err(()),
    }
}

/// Sanitizer you already had; exported so server.rs can keep using it.
pub fn sanitize_filename_component(raw: &str) -> String {
    raw.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

impl ScadParamTemplate {
    /// Read `main_path` and any `include <...>` / `use <...>` recursively.
    pub fn from_scad_tree(main_path: &Path) -> anyhow::Result<Self> {
        let mut visited = HashSet::<PathBuf>::new();
        let mut all_text = String::new();
        gather_scad_text(main_path, &mut visited, &mut all_text)?;

        let specs_vec = extract_param_specs(&all_text);
        let mut specs = BTreeMap::new();
        let mut defaults = BTreeMap::new();

        for spec in specs_vec {
            defaults.insert(spec.name.clone(), spec.default.clone());
            specs.insert(spec.name.clone(), spec);
        }

        Ok(Self { specs, defaults })
    }

    /// Per request, start with discovered defaults.
    pub fn instantiate(&self) -> ScadParams {
        ScadParams {
            specs: self.specs.clone(),
            values: self.defaults.clone(),
        }
    }

    /// list of user-facing names to build the HTML form with
    pub fn user_param_names(&self) -> impl Iterator<Item = &str> {
        self.specs
            .values()
            .filter(|s| s.is_user_param)
            .map(|s| s.name.as_str())
    }
}

impl ScadParams {
    /// Update from a multipart field if it matches a discovered param.
    /// Field names in form are expected to be snake_case; SCAD vars are CAPS.
    pub fn set_from_field(&mut self, field_name: &str, text: &str) -> Result<(), ()> {
        if text.trim().is_empty() {
            return Ok(());
        }
        let scad_name = field_to_scad_name(field_name);

        let Some(spec) = self.specs.get(&scad_name) else {
            // Unknown field; ignore quietly (matches your old behavior).
            return Ok(());
        };

        let v = match spec.ty {
            ParamType::Bool => {
                let b = parse_bool(text)?;
                if b { "true" } else { "false" }.to_string()
            }
            ParamType::Number => {
                // Validate numeric; keep original string for SCAD.
                text.parse::<f64>().map_err(|_| ())?;
                text.to_string()
            }
            ParamType::String => {
                // Escape quotes/backslashes minimally, then wrap.
                let esc = text.replace('\\', "\\\\").replace('"', "\\\"");
                format!("\"{}\"", esc)
            }
        };

        self.values.insert(scad_name, v);
        Ok(())
    }

    /// Iterate "-D NAME=value" fragments in stable order.
    pub fn iter_defines(&self) -> impl Iterator<Item = String> + '_ {
        self.values.iter().map(|(k, v)| format!("{k}={v}"))
    }

    /// Convenience for server code (e.g. NAME used for output filename)
    pub fn get_raw(&self, scad_name: &str) -> Option<&String> {
        self.values.get(scad_name)
    }
}

// -------- internals --------

fn field_to_scad_name(field: &str) -> String {
    field.to_ascii_uppercase()
}

/// Recursively gather text from main file and its includes.
fn gather_scad_text(
    path: &Path,
    visited: &mut HashSet<PathBuf>,
    out: &mut String,
) -> anyhow::Result<()> {
    let canon = path
        .canonicalize()
        .with_context(|| format!("canonicalize {}", path.display()))?;
    if !visited.insert(canon.clone()) {
        return Ok(());
    }

    let text = fs::read_to_string(&canon).with_context(|| format!("read {}", canon.display()))?;
    out.push_str(&text);
    out.push('\n');

    let dir = canon.parent().unwrap_or(Path::new("."));

    // include <foo.scad>;
    let include_re = Regex::new(r#"(?m)^\s*(?:include|use)\s*<([^>]+)>\s*;"#).unwrap();
    for cap in include_re.captures_iter(&text) {
        let rel = cap[1].trim();
        let inc = dir.join(rel);
        if inc.exists() {
            gather_scad_text(&inc, visited, out)?;
        }
    }

    Ok(())
}

/// Extract `FOO = bar;` defaults.
/// Also supports optional `// @param` marker to control user-facing params.
///
/// Heuristics:
/// - Only ALL_CAPS identifiers are treated as params.
/// - `$fn` etc are ignored.
/// - If any line uses `// @param`, we *only* accept marked lines as user params.
///   (this prevents derived globals like FIT, LOGO_TARGET from becoming overrideable)
pub fn extract_param_specs(text: &str) -> Vec<ParamSpec> {
    // Capture: NAME = RHS;  // optional comment
    let assign_re =
        Regex::new(r#"(?m)^\s*([A-Z][A-Z0-9_]*)\s*=\s*([^;]+);\s*(?://\s*(.*))?$"#).unwrap();

    let mut raws = Vec::new();
    let mut any_marked = false;

    for cap in assign_re.captures_iter(text) {
        let name = cap[1].to_string();
        if name.starts_with('$') {
            continue;
        }
        let rhs = cap[2].trim().to_string();
        let comment = cap
            .get(3)
            .map(|m| m.as_str())
            .unwrap_or("")
            .trim()
            .to_string();
        let is_marked = comment.contains("@param");
        if is_marked {
            any_marked = true;
        }
        raws.push((name, rhs, is_marked, comment));
    }

    raws.into_iter()
        .filter_map(|(name, rhs, is_marked, comment)| {
            let is_user_param = if any_marked { is_marked } else { true };

            let ty = if rhs.trim_start().starts_with('"') {
                ParamType::String
            } else {
                match rhs.to_ascii_lowercase().trim() {
                    "true" | "false" => ParamType::Bool,
                    _ => ParamType::Number,
                }
            };

            let options = parse_options_from_comment(&comment);

            Some(ParamSpec {
                name,
                default: rhs,
                ty,
                is_user_param,
                comment,
                options,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_finds_caps_assignments_and_types() {
        let scad = r#"
MODE = "base";
COASTER_D = 101.6;
USE_SPINNER = true;
FIT = CLEARANCE/2;
$fn = 200;
"#;

        let specs = extract_param_specs(scad);
        let names: Vec<_> = specs.iter().map(|s| s.name.as_str()).collect();

        assert!(names.contains(&"MODE"));
        assert!(names.contains(&"COASTER_D"));
        assert!(names.contains(&"USE_SPINNER"));
        assert!(names.contains(&"FIT"));
        assert!(!names.contains(&"$fn"));

        let mode = specs.iter().find(|s| s.name == "MODE").unwrap();
        assert_eq!(mode.ty, ParamType::String);

        let d = specs.iter().find(|s| s.name == "COASTER_D").unwrap();
        assert_eq!(d.ty, ParamType::Number);

        let b = specs.iter().find(|s| s.name == "USE_SPINNER").unwrap();
        assert_eq!(b.ty, ParamType::Bool);
    }

    #[test]
    fn marker_filtering_works() {
        let scad = r#"
MODE = "base";  // @param
COASTER_D = 101.6; // @param
FIT = CLEARANCE/2;
"#;
        let specs = extract_param_specs(scad);
        let user: Vec<_> = specs
            .iter()
            .filter(|s| s.is_user_param)
            .map(|s| s.name.as_str())
            .collect();

        assert_eq!(user, vec!["MODE", "COASTER_D"]);
    }

    #[test]
    fn set_from_field_only_updates_known_params() {
        let scad = r#"
MODE = "base";
COASTER_D = 101.6;
USE_SPINNER = true;
"#;
        let _tmpl = ScadParamTemplate::from_scad_tree(
            Path::new("/dev/null"), // not used here
        );

        // cheat: build directly from text
        let specs_vec = extract_param_specs(scad);
        let mut specs = BTreeMap::new();
        let mut defaults = BTreeMap::new();
        for s in specs_vec {
            defaults.insert(s.name.clone(), s.default.clone());
            specs.insert(s.name.clone(), s);
        }
        let tmpl = ScadParamTemplate { specs, defaults };

        let mut p = tmpl.instantiate();
        p.set_from_field("mode", "preview").unwrap();
        p.set_from_field("use_spinner", "false").unwrap();
        p.set_from_field("unknown", "123").unwrap();

        assert_eq!(p.get_raw("MODE").unwrap(), "\"preview\"");
        assert_eq!(p.get_raw("USE_SPINNER").unwrap(), "false");
        assert!(p.get_raw("UNKNOWN").is_none());
    }
}

fn parse_options_from_comment(comment: &str) -> Vec<String> {
    // Accept e.g.:
    //   // @param options: base|inlay|magnet|preview
    //   // options: octagon, circle
    let lower = comment.to_ascii_lowercase();
    let Some(idx) = lower.find("options:") else {
        return Vec::new();
    };

    let rest = &comment[idx + "options:".len()..];
    rest.split(|c: char| c == '|' || c == ',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

#[test]
fn options_parse_from_comment() {
    let scad = r#"
MODE="base"; // @param options: base|inlay|magnet|preview
"#;
    let specs = extract_param_specs(scad);
    let mode = specs.iter().find(|s| s.name == "MODE").unwrap();
    assert_eq!(mode.options, vec!["base", "inlay", "magnet", "preview"]);
}
