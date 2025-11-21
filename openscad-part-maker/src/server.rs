use axum::{
    extract::{Multipart, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::{Html, IntoResponse, Response},
    routing::{get, post},
    Router,
};
use log::{debug, error, info};
use std::{net::SocketAddr, path::PathBuf, sync::Arc};
use tempfile::tempdir;
use tokio::{net::TcpListener, process::Command};

use crate::scad_params::{sanitize_filename_component, ParamType, ScadParamTemplate, ScadParams};

#[derive(Clone)]
pub struct AppState {
    pub input_scad_path: PathBuf,
    pub scad_template: ScadParamTemplate,
}

pub async fn run(addr: SocketAddr, input_scad_path: PathBuf) -> anyhow::Result<()> {
    let scad_template = ScadParamTemplate::from_scad_tree(&input_scad_path)?;

    let state = Arc::new(AppState {
        input_scad_path,
        scad_template,
    });

    let app = Router::new()
        .route("/", get(index))
        .route("/render", post(render_svg_to_stl))
        .with_state(state);

    let listener = TcpListener::bind(addr).await?;
    info!("Starting HTTP server on http://{}", listener.local_addr()?);

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    info!("HTTP server shut down gracefully");
    Ok(())
}

async fn index(State(state): State<Arc<AppState>>) -> Html<String> {
    Html(build_index_html(&state.scad_template))
}

/// Generate the index HTML using discovered SCAD parameters.
/// `fs/fa/fn` stay static. Everything in template that is_user_param becomes a field.
fn build_index_html(template: &ScadParamTemplate) -> String {
    let mut param_fields = String::new();

    // We keep NAME as a special required field right after SVG upload.
    let have_name = template
        .specs
        .get("NAME")
        .map(|s| s.is_user_param)
        .unwrap_or(false);

    for spec in template.specs.values() {
        if !spec.is_user_param {
            continue;
        }

        // NAME is handled specially above; SVG_PATH is always overridden by upload.
        if spec.name == "NAME" || spec.name == "SVG_PATH" {
            continue;
        }

        let field_name = spec.name.to_ascii_lowercase(); // snake-ish already
        let label = humanize_scad_name(&spec.name);
        let default_unquoted = unquote_if_string(&spec.default);

        match spec.ty {
            ParamType::Bool => {
                let checked = if spec.default.trim().eq_ignore_ascii_case("true") {
                    "checked"
                } else {
                    ""
                };
                param_fields.push_str(&format!(
                    r#"
      <div class="field-row checkbox-row">
        <span></span>
        <label class="checkbox-label">
          <input id="{id}" type="checkbox" name="{name}" {checked}>
          {label}
        </label>
      </div>
"#,
                    id = field_name,
                    name = field_name,
                    label = html_escape(&label),
                    checked = checked
                ));
            }
            ParamType::Number => {
                let step = if default_unquoted.contains('.') {
                    "0.1"
                } else {
                    "1"
                };
                param_fields.push_str(&format!(
                    r#"
      <div class="field-row">
        <label for="{id}">{label}</label>
        <input id="{id}" type="number" step="{step}" name="{name}" value="{val}">
      </div>
"#,
                    id = field_name,
                    name = field_name,
                    label = html_escape(&label),
                    step = step,
                    val = html_escape(&default_unquoted)
                ));
            }
            ParamType::String => {
                // If comment declared options, use them. Otherwise fall back for known enums.
                let mut options = spec.options.clone();
                if options.is_empty() {
                    options = match spec.name.as_str() {
                        "MODE" => vec!["base", "inlay", "magnet", "preview"]
                            .into_iter()
                            .map(|s| s.to_string())
                            .collect(),
                        "SHAPE" => vec!["octagon", "circle"]
                            .into_iter()
                            .map(|s| s.to_string())
                            .collect(),
                        _ => Vec::new(),
                    };
                }

                if !options.is_empty() {
                    let opts_html = options
                        .iter()
                        .map(|opt| {
                            let selected = if opt == &default_unquoted {
                                " selected"
                            } else {
                                ""
                            };
                            format!(
                                r#"          <option value="{v}"{sel}>{label}</option>"#,
                                v = html_escape(opt),
                                label = html_escape(opt),
                                sel = selected
                            )
                        })
                        .collect::<Vec<_>>()
                        .join("\n");

                    param_fields.push_str(&format!(
                        r#"
      <div class="field-row">
        <label for="{id}">{label}</label>
        <select id="{id}" name="{name}">
{opts}
        </select>
      </div>
"#,
                        id = field_name,
                        name = field_name,
                        label = html_escape(&label),
                        opts = opts_html
                    ));
                } else {
                    // No options → plain text input.
                    param_fields.push_str(&format!(
                        r#"
      <div class="field-row">
        <label for="{id}">{label}</label>
        <input id="{id}" type="text" name="{name}" value="{val}">
      </div>
"#,
                        id = field_name,
                        name = field_name,
                        label = html_escape(&label),
                        val = html_escape(&default_unquoted)
                    ));
                }
            }
        }
    }

    // If NAME is not a marked param, we still show it (previous UX).
    let name_field = if have_name || template.specs.get("NAME").is_some() {
        r#"
      <!-- 2. Name (required, auto-filled from SVG) -->
      <div class="field-row">
        <label for="name">Name</label>
        <input
          id="name"
          type="text"
          name="name"
          placeholder="Name"
          required
        >
      </div>
"#
        .to_string()
    } else {
        // Still show it even if not in specs; it will be used for filename + -D NAME
        r#"
      <!-- 2. Name (required, auto-filled from SVG) -->
      <div class="field-row">
        <label for="name">Name</label>
        <input
          id="name"
          type="text"
          name="name"
          placeholder="Name"
          required
        >
      </div>
"#
        .to_string()
    };

    format!(
        r#"<!DOCTYPE html>
<html>
<head>
  <meta charset="utf-8">
  <title>OpenSCAD STL Generator</title>
  <style>
    :root {{
      color-scheme: dark light;
      --bg: #0f172a;
      --fg: #e5e7eb;
      --card-bg: #020617;
      --accent: #22c55e;
      --accent-hover: #16a34a;
      --border: #1f2937;
      --input-bg: #020617;
    }}
    body {{
      margin: 0;
      font-family: system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
      background: radial-gradient(circle at top, #1f2937, #020617);
      color: var(--fg);
      min-height: 100vh;
      display: flex;
      align-items: center;
      justify-content: center;
      padding: 1.5rem;
    }}
    .card {{
      background: rgba(2, 6, 23, 0.95);
      border: 1px solid var(--border);
      border-radius: 1rem;
      padding: 1.5rem 1.75rem;
      max-width: 720px;
      width: 100%;
      box-shadow: 0 24px 60px rgba(0, 0, 0, 0.6);
      backdrop-filter: blur(12px);
    }}
    h1 {{
      margin: 0 0 0.75rem;
      font-size: 1.4rem;
      text-align: center;
    }}
    p.subtitle {{
      margin: 0 0 1.5rem;
      color: #9ca3af;
      font-size: 0.95rem;
      text-align: center;
    }}
    form {{
      margin-top: 0.5rem;
    }}
    .field-row {{
      display: grid;
      grid-template-columns: 180px minmax(0, 1fr);
      column-gap: 0.75rem;
      align-items: center;
      margin-bottom: 0.6rem;
    }}
    .field-row label {{
      font-size: 0.85rem;
      color: #d1d5db;
    }}
    .field-row input,
    .field-row select {{
      padding: 0.45rem 0.6rem;
      border-radius: 0.5rem;
      border: 1px solid var(--border);
      background-color: var(--input-bg);
      color: var(--fg);
      font-size: 0.9rem;
      outline: none;
      transition: border-color 0.15s ease, box-shadow 0.15s ease;
      width: 100%;
      box-sizing: border-box;
    }}
    .field-row input[type="file"] {{
      padding: 0.3rem;
    }}
    .field-row input:focus,
    .field-row select:focus {{
      border-color: var(--accent);
      box-shadow: 0 0 0 1px rgba(34, 197, 94, 0.4);
    }}
    .section-title {{
      margin: 0.9rem 0 0.15rem;
      font-size: 0.9rem;
      font-weight: 600;
      color: #9ca3af;
      text-transform: uppercase;
      letter-spacing: 0.05em;
    }}
    .section-divider {{
      height: 1px;
      border: none;
      background: linear-gradient(to right, transparent, #1f2937, transparent);
      margin: 0 0 0.6rem;
    }}
    .checkbox-row {{
      grid-template-columns: 180px minmax(0, 1fr);
    }}
    .checkbox-label {{
      display: inline-flex;
      align-items: center;
      gap: 0.45rem;
      font-size: 0.9rem;
      color: #d1d5db;
    }}
    .checkbox-label input[type="checkbox"] {{
      width: 1rem;
      height: 1rem;
      accent-color: var(--accent);
    }}
    button[type="submit"] {{
      margin-top: 0.8rem;
      width: 100%;
      padding: 0.55rem 0.75rem;
      border-radius: 9999px;
      border: none;
      background: radial-gradient(circle at top left, #4ade80, var(--accent));
      color: #022c22;
      font-weight: 600;
      font-size: 0.95rem;
      cursor: pointer;
      box-shadow: 0 14px 30px rgba(22, 163, 74, 0.45);
      transition: transform 0.1s ease, box-shadow 0.1s ease, filter 0.1s ease;
    }}
    button[type="submit"]:hover {{
      filter: brightness(1.05);
      box-shadow: 0 18px 40px rgba(22, 163, 74, 0.7);
      transform: translateY(-1px);
    }}
    button[type="submit"]:active {{
      transform: translateY(1px);
      box-shadow: 0 10px 22px rgba(22, 163, 74, 0.6);
    }}
    .hint {{
      margin-top: 0.5rem;
      font-size: 0.8rem;
      color: #9ca3af;
      text-align: center;
    }}
  </style>
</head>
<body>
  <div class="card">
    <h1>Generate STL from SVG</h1>
    <p class="subtitle">
      Upload a logo SVG and tweak the OpenSCAD parameters to generate a printable coaster STL.
    </p>

    <form action="/render" method="post" enctype="multipart/form-data">
      <!-- 1. SVG file -->
      <div class="field-row">
        <label for="svg">SVG file</label>
        <input id="svg" type="file" name="svg" accept=".svg" required>
      </div>

{NAME_FIELD}

      <div class="section-title">OpenSCAD quality</div>
      <hr class="section-divider">

      <div class="field-row">
        <label for="fs">fs (min size)</label>
        <input id="fs" type="number" step="0.01" name="fs" value="0.1">
      </div>
      <div class="field-row">
        <label for="fa">fa (angle)</label>
        <input id="fa" type="number" step="1" name="fa" value="5">
      </div>
      <div class="field-row">
        <label for="fn">fn (segments)</label>
        <input id="fn" type="number" step="1" name="fn" value="200">
      </div>

      <div class="section-title">OpenSCAD parameters</div>
      <hr class="section-divider">

{PARAM_FIELDS}

      <button type="submit">Generate STL</button>
      <div class="hint">
        STL will download automatically once OpenSCAD finishes rendering.
      </div>
    </form>
  </div>

  <script>
    (function () {{
      const fileInput = document.getElementById('svg');
      const nameInput = document.getElementById('name');
      if (!fileInput || !nameInput) return;

      let lastAutoName = "";

      fileInput.addEventListener('change', function () {{
        const file = this.files && this.files[0];
        if (!file) return;

        const fullName = file.name || "";
        const dot = fullName.lastIndexOf('.');
        const base = dot > 0 ? fullName.slice(0, dot) : fullName;

        // Only overwrite if the field is empty or matches the last auto-filled name
        if (nameInput.value.trim() === "" || nameInput.value === lastAutoName) {{
          nameInput.value = base;
          lastAutoName = base;
        }}
      }});
    }})();
  </script>
</body>
</html>
"#,
        NAME_FIELD = name_field,
        PARAM_FIELDS = param_fields
    )
}

fn humanize_scad_name(name: &str) -> String {
    // "COASTER_D" -> "Coaster D", "BOTTOM_SKIN" -> "Bottom Skin"
    name.split('_')
        .map(|w| {
            let mut chars = w.chars();
            match chars.next() {
                None => String::new(),
                Some(c) => {
                    let rest = chars.as_str().to_ascii_lowercase();
                    format!("{}{}", c.to_ascii_uppercase(), rest)
                }
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn unquote_if_string(rhs: &str) -> String {
    let t = rhs.trim();
    if t.starts_with('"') && t.ends_with('"') && t.len() >= 2 {
        t[1..t.len() - 1].to_string()
    } else {
        t.to_string()
    }
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

/// POST /render – accepts multipart form with an SVG file and params, returns STL.
async fn render_svg_to_stl(
    State(state): State<Arc<AppState>>,
    mut multipart: Multipart,
) -> Result<Response, StatusCode> {
    let mut svg_bytes: Option<bytes::Bytes> = None;

    // OpenSCAD "quality" params:
    let mut fs: f32 = 0.1;
    let mut fa: f32 = 5.0;
    let mut fn_: i32 = 200;

    // Discovered params:
    let mut scad_params = state.scad_template.instantiate();
    let mut form_name: Option<String> = None;

    while let Some(field) = multipart.next_field().await.map_err(|err| {
        error!("Failed to read multipart field: {err}");
        StatusCode::BAD_REQUEST
    })? {
        let name = field.name().unwrap_or("").to_string();
        debug!("Received multipart field: {name}");

        if name == "svg" {
            let bytes = field.bytes().await.map_err(|err| {
                error!("Failed to read svg field: {err}");
                StatusCode::BAD_REQUEST
            })?;
            svg_bytes = Some(bytes);
            continue;
        }

        // Everything else: treat as text field
        let text = field.text().await.map_err(|err| {
            error!("Failed to read text field {name}: {err}");
            StatusCode::BAD_REQUEST
        })?;

        match name.as_str() {
            "fs" => {
                if !text.is_empty() {
                    fs = text.parse().map_err(|_| StatusCode::BAD_REQUEST)?;
                }
            }
            "fa" => {
                if !text.is_empty() {
                    fa = text.parse().map_err(|_| StatusCode::BAD_REQUEST)?;
                }
            }
            "fn" => {
                if !text.is_empty() {
                    fn_ = text.parse().map_err(|_| StatusCode::BAD_REQUEST)?;
                }
            }
            "name" => {
                // Keep old UX: always accept name, even if not in scad defaults.
                form_name = Some(text.clone());
                scad_params
                    .set_from_field(&name, &text)
                    .map_err(|_| StatusCode::BAD_REQUEST)?;
            }
            _ => {
                scad_params
                    .set_from_field(&name, &text)
                    .map_err(|_| StatusCode::BAD_REQUEST)?;
            }
        }
    }

    // Force NAME into params if user gave one, even if template lacks a NAME spec.
    if let Some(n) = form_name {
        let esc = n.replace('\\', "\\\\").replace('"', "\\\"");
        scad_params
            .values
            .insert("NAME".into(), format!("\"{}\"", esc));
    }

    let svg_bytes = svg_bytes.ok_or(StatusCode::BAD_REQUEST)?;

    // temp dir, write SVG
    let tmpdir = tempdir().map_err(|err| {
        error!("Failed to create temp dir: {err}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    let svg_path = tmpdir.path().join("input.svg");
    let stl_path = tmpdir.path().join("output.stl");

    tokio::fs::write(&svg_path, &svg_bytes)
        .await
        .map_err(|err| {
            error!("Failed to write SVG to disk: {err}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let args = build_openscad_args(
        fs,
        fa,
        fn_,
        &scad_params,
        &svg_path,
        &stl_path,
        &state.input_scad_path,
    );
    let mut cmd = Command::new("openscad");
    cmd.args(args);

    info!("Running openscad to generate STL...");
    let status = cmd.status().await.map_err(|err| {
        error!("Failed to spawn openscad: {err}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    if !status.success() {
        error!("openscad exited with non-zero status: {status}");
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    }

    let stl_bytes = tokio::fs::read(&stl_path).await.map_err(|err| {
        error!("Failed to read generated STL: {err}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let mut headers = HeaderMap::new();

    let safe_name = sanitize_filename_component(
        &scad_params
            .get_raw("NAME")
            .and_then(|s| s.strip_prefix('"'))
            .and_then(|s| s.strip_suffix('"'))
            .unwrap_or("output"),
    );

    headers.insert(header::CONTENT_TYPE, HeaderValue::from_static("model/stl"));

    let disposition = format!("attachment; filename=\"{safe_name}.stl\"");
    let disposition_value = HeaderValue::from_str(&disposition).map_err(|err| {
        error!("Invalid Content-Disposition header value: {err}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    headers.insert(header::CONTENT_DISPOSITION, disposition_value);

    Ok((headers, stl_bytes).into_response())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        use tokio::signal::unix::{signal, SignalKind};
        signal(SignalKind::terminate())
            .expect("failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {
            info!("SIGINT (Ctrl+C) received, shutting down…");
        }
        _ = terminate => {
            info!("SIGTERM received, shutting down…");
        }
    }
}

fn build_openscad_args(
    fs: f32,
    fa: f32,
    fn_: i32,
    scad_params: &ScadParams,
    svg_path: &PathBuf,
    stl_path: &PathBuf,
    input_scad_path: &PathBuf,
) -> Vec<String> {
    let mut args = Vec::new();

    args.push("--render".into());
    args.push("-D".into());
    args.push(format!("fs={fs}"));
    args.push("-D".into());
    args.push(format!("fa={fa}"));
    args.push("-D".into());
    args.push(format!("fn={fn_}"));

    for define in scad_params.iter_defines() {
        args.push("-D".into());
        args.push(define);
    }

    args.push("-D".into());
    args.push(format!("SVG_PATH=\"{}\"", svg_path.display()));

    args.push("-o".into());
    args.push(stl_path.to_string_lossy().to_string());
    args.push(input_scad_path.to_string_lossy().to_string());

    args
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scad_params::{extract_param_specs, parse_bool, ScadParamTemplate};

    #[test]
    fn parse_bool_accepts_truthy_variants() {
        for v in ["1", "true", "TRUE", "on", "On", "yes", "YeS"] {
            assert_eq!(parse_bool(v), Ok(true), "variant {v} should be true");
        }
    }

    #[test]
    fn parse_bool_accepts_falsy_variants() {
        for v in ["0", "false", "FALSE", "off", "Off", "no", "No"] {
            assert_eq!(parse_bool(v), Ok(false), "variant {v} should be false");
        }
    }

    #[test]
    fn parse_bool_rejects_unknown_strings() {
        for v in ["", "maybe", "truth", "2", "yep", "nah"] {
            assert!(parse_bool(v).is_err(), "variant {v} should be Err");
        }
    }

    #[test]
    fn sanitize_filename_component_replaces_unsafe_chars() {
        assert_eq!(sanitize_filename_component("My Logo!.svg"), "My_Logo__svg");
        assert_eq!(
            sanitize_filename_component("weird/\\name?*"),
            "weird__name__"
        );
        assert_eq!(sanitize_filename_component("ümlaut💀"), "_mlaut_");
    }

    #[test]
    fn build_openscad_args_contains_expected_params_and_order() {
        let scad = r#"
NAME="output"; // @param
MODE="base"; // @param
SHAPE="octagon"; // @param
INTERLOCK=false; // @param
USE_SPINNER=true; // @param
"#;

        let specs = extract_param_specs(scad);
        let mut map = std::collections::BTreeMap::new();
        let mut defaults = std::collections::BTreeMap::new();
        for s in specs {
            defaults.insert(s.name.clone(), s.default.clone());
            map.insert(s.name.clone(), s);
        }
        let tmpl = ScadParamTemplate {
            specs: map,
            defaults,
        };

        let mut p = tmpl.instantiate();
        p.set_from_field("name", "My Logo").unwrap();
        p.set_from_field("mode", "preview").unwrap();
        p.set_from_field("shape", "circle").unwrap();
        p.set_from_field("interlock", "true").unwrap();
        p.set_from_field("use_spinner", "false").unwrap();

        let svg = PathBuf::from("/tmp/input.svg");
        let stl = PathBuf::from("/tmp/output.stl");
        let main_scad = PathBuf::from("/app/input.scad");

        let args = build_openscad_args(0.25, 9.0, 123, &p, &svg, &stl, &main_scad);

        assert_eq!(args[0], "--render");
        assert_eq!(args[1], "-D");
        assert_eq!(args[2], "fs=0.25");
        assert!(args.contains(&"NAME=\"My Logo\"".to_string()));
        assert!(args.contains(&"MODE=\"preview\"".to_string()));
        assert!(args.contains(&"SHAPE=\"circle\"".to_string()));
        assert!(args.contains(&"INTERLOCK=true".to_string()));
        assert!(args.contains(&"USE_SPINNER=false".to_string()));
    }

    #[test]
    fn build_index_html_renders_discovered_fields() {
        let scad = r#"
NAME="output"; // @param
COASTER_D=101.6; // @param
USE_SPINNER=true; // @param
"#;

        let specs = extract_param_specs(scad);
        let mut map = std::collections::BTreeMap::new();
        let mut defaults = std::collections::BTreeMap::new();
        for s in specs {
            defaults.insert(s.name.clone(), s.default.clone());
            map.insert(s.name.clone(), s);
        }
        let tmpl = ScadParamTemplate {
            specs: map,
            defaults,
        };

        let html = build_index_html(&tmpl);

        assert!(html.contains("<form action=\"/render\""));
        assert!(html.contains("name=\"svg\""));
        assert!(html.contains("name=\"name\""));
        assert!(html.contains("name=\"coaster_d\""));
        assert!(html.contains("name=\"use_spinner\""));
        assert!(html.contains("OpenSCAD parameters"));
    }
}
