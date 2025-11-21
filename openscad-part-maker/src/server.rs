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

#[derive(Clone)]
pub struct AppState {
    pub input_scad_path: PathBuf,
}

#[derive(Debug, Clone)]
struct LibParams {
    name: String,
    mode: String,
    shape: String,
    shape_rot: f32,
    coaster_d: f32,
    base_h: f32,
    inlay_dh: f32,
    margin: f32,
    clearance: f32,
    seg: i32,
    interlock: bool,
    edge_clear: f32,
    mag_d: f32,
    mag_h: f32,
    mag_clear: f32,
    bottom_skin: f32,
    boss_clearance: f32,
    boss_h: f32,
    spinner_d: f32,
    use_spinner: bool,
}

impl Default for LibParams {
    fn default() -> Self {
        Self {
            name: "output".into(),
            mode: "base".into(),
            shape: "octagon".into(),
            shape_rot: 22.5,
            coaster_d: 101.6,
            base_h: 5.0,
            inlay_dh: 1.2,
            margin: 27.5,
            clearance: 0.10,
            seg: 200,
            interlock: false,
            edge_clear: 15.0,
            mag_d: 15.5,
            mag_h: 3.0,
            mag_clear: 0.5,
            bottom_skin: 0.4,
            boss_clearance: 0.2,
            boss_h: 0.8,
            spinner_d: 15.0,
            use_spinner: true,
        }
    }
}

fn parse_bool(value: &str) -> Result<bool, ()> {
    match value.to_ascii_lowercase().as_str() {
        "1" | "true" | "on" | "yes" => Ok(true),
        "0" | "false" | "off" | "no" => Ok(false),
        _ => Err(()),
    }
}

impl LibParams {
    fn set_from_field(&mut self, name: &str, text: &str) -> Result<(), ()> {
        if text.is_empty() {
            return Ok(());
        }
        match name {
            "name" => self.name = text.to_string(),
            "mode" => self.mode = text.to_string(),
            "shape" => self.shape = text.to_string(),
            "shape_rot" => self.shape_rot = text.parse().map_err(|_| ())?,
            "coaster_d" => self.coaster_d = text.parse().map_err(|_| ())?,
            "base_h" => self.base_h = text.parse().map_err(|_| ())?,
            "inlay_dh" => self.inlay_dh = text.parse().map_err(|_| ())?,
            "margin" => self.margin = text.parse().map_err(|_| ())?,
            "clearance" => self.clearance = text.parse().map_err(|_| ())?,
            "seg" => self.seg = text.parse().map_err(|_| ())?,
            "interlock" => self.interlock = parse_bool(text)?,
            "edge_clear" => self.edge_clear = text.parse().map_err(|_| ())?,
            "mag_d" => self.mag_d = text.parse().map_err(|_| ())?,
            "mag_h" => self.mag_h = text.parse().map_err(|_| ())?,
            "mag_clear" => self.mag_clear = text.parse().map_err(|_| ())?,
            "bottom_skin" => self.bottom_skin = text.parse().map_err(|_| ())?,
            "boss_clearance" => self.boss_clearance = text.parse().map_err(|_| ())?,
            "boss_h" => self.boss_h = text.parse().map_err(|_| ())?,
            "spinner_d" => self.spinner_d = text.parse().map_err(|_| ())?,
            "use_spinner" => self.use_spinner = parse_bool(text)?,
            _ => {}
        }
        Ok(())
    }
}

pub async fn run(addr: SocketAddr, input_scad_path: PathBuf) -> anyhow::Result<()> {
    let state = Arc::new(AppState { input_scad_path });

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

async fn index() -> Html<&'static str> {
    Html(
        r#"<!DOCTYPE html>
<html>
<head>
  <meta charset="utf-8">
  <title>OpenSCAD STL Generator</title>
  <style>
    :root {
      color-scheme: dark light;
      --bg: #0f172a;
      --fg: #e5e7eb;
      --card-bg: #020617;
      --accent: #22c55e;
      --accent-hover: #16a34a;
      --border: #1f2937;
      --input-bg: #020617;
    }
    body {
      margin: 0;
      font-family: system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
      background: radial-gradient(circle at top, #1f2937, #020617);
      color: var(--fg);
      min-height: 100vh;
      display: flex;
      align-items: center;
      justify-content: center;
      padding: 1.5rem;
    }
    .card {
      background: rgba(2, 6, 23, 0.95);
      border: 1px solid var(--border);
      border-radius: 1rem;
      padding: 1.5rem 1.75rem;
      max-width: 720px;
      width: 100%;
      box-shadow: 0 24px 60px rgba(0, 0, 0, 0.6);
      backdrop-filter: blur(12px);
    }
    h1 {
      margin: 0 0 0.75rem;
      font-size: 1.4rem;
      text-align: center;
    }
    p.subtitle {
      margin: 0 0 1.5rem;
      color: #9ca3af;
      font-size: 0.95rem;
      text-align: center;
    }
    form {
      margin-top: 0.5rem;
    }
    .field-row {
      display: grid;
      grid-template-columns: 180px minmax(0, 1fr);
      column-gap: 0.75rem;
      align-items: center;
      margin-bottom: 0.6rem;
    }
    .field-row label {
      font-size: 0.85rem;
      color: #d1d5db;
    }
    .field-row input,
    .field-row select {
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
    }
    .field-row input[type="file"] {
      padding: 0.3rem;
    }
    .field-row input:focus,
    .field-row select:focus {
      border-color: var(--accent);
      box-shadow: 0 0 0 1px rgba(34, 197, 94, 0.4);
    }
    .section-title {
      margin: 0.9rem 0 0.15rem;
      font-size: 0.9rem;
      font-weight: 600;
      color: #9ca3af;
      text-transform: uppercase;
      letter-spacing: 0.05em;
    }
    .section-divider {
      height: 1px;
      border: none;
      background: linear-gradient(to right, transparent, #1f2937, transparent);
      margin: 0 0 0.6rem;
    }
    .checkbox-row {
      grid-template-columns: 180px minmax(0, 1fr);
    }
    .checkbox-label {
      display: inline-flex;
      align-items: center;
      gap: 0.45rem;
      font-size: 0.9rem;
      color: #d1d5db;
    }
    .checkbox-label input[type="checkbox"] {
      width: 1rem;
      height: 1rem;
      accent-color: var(--accent);
    }
    button[type="submit"] {
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
    }
    button[type="submit"]:hover {
      filter: brightness(1.05);
      box-shadow: 0 18px 40px rgba(22, 163, 74, 0.7);
      transform: translateY(-1px);
    }
    button[type="submit"]:active {
      transform: translateY(1px);
      box-shadow: 0 10px 22px rgba(22, 163, 74, 0.6);
    }
    .hint {
      margin-top: 0.5rem;
      font-size: 0.8rem;
      color: #9ca3af;
      text-align: center;
    }
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

      <div class="section-title">Coaster parameters (lib.scad)</div>
      <hr class="section-divider">

      <div class="field-row">
        <label for="mode">Mode</label>
        <select id="mode" name="mode">
          <option value="base" selected>base</option>
          <option value="inlay">inlay</option>
          <option value="magnet">magnet</option>
          <option value="preview">preview</option>
        </select>
      </div>
      <div class="field-row">
        <label for="shape">Shape</label>
        <select id="shape" name="shape">
          <option value="octagon" selected>octagon</option>
          <option value="circle">circle</option>
        </select>
      </div>
      <div class="field-row">
        <label for="shape_rot">Shape rotation (deg)</label>
        <input id="shape_rot" type="number" step="0.1" name="shape_rot" value="22.5">
      </div>
      <div class="field-row">
        <label for="coaster_d">Coaster diameter (mm)</label>
        <input id="coaster_d" type="number" step="0.1" name="coaster_d" value="101.6">
      </div>
      <div class="field-row">
        <label for="base_h">Base height (mm)</label>
        <input id="base_h" type="number" step="0.1" name="base_h" value="5">
      </div>
      <div class="field-row">
        <label for="inlay_dh">Inlay depth (mm)</label>
        <input id="inlay_dh" type="number" step="0.1" name="inlay_dh" value="1.2">
      </div>
      <div class="field-row">
        <label for="margin">Margin (mm)</label>
        <input id="margin" type="number" step="0.1" name="margin" value="27.5">
      </div>
      <div class="field-row">
        <label for="clearance">Clearance (mm)</label>
        <input id="clearance" type="number" step="0.01" name="clearance" value="0.10">
      </div>
      <div class="field-row">
        <label for="seg">Segments</label>
        <input id="seg" type="number" step="1" name="seg" value="200">
      </div>
      <div class="field-row checkbox-row">
        <span></span>
        <label class="checkbox-label">
          <input id="interlock" type="checkbox" name="interlock">
          Interlock
        </label>
      </div>
      <div class="field-row">
        <label for="edge_clear">Edge clear (mm)</label>
        <input id="edge_clear" type="number" step="0.1" name="edge_clear" value="15">
      </div>

      <div class="section-title">Spinner</div>
      <hr class="section-divider">

      <div class="field-row">
        <label for="spinner_d">Spinner diameter (mm)</label>
        <input id="spinner_d" type="number" step="0.1" name="spinner_d" value="15">
      </div>
      <div class="field-row checkbox-row">
        <span></span>
        <label class="checkbox-label">
          <input id="use_spinner" type="checkbox" name="use_spinner" checked>
          Use spinner hole
        </label>
      </div>

      <button type="submit">Generate STL</button>
      <div class="hint">
        STL will download automatically once OpenSCAD finishes rendering.
      </div>
    </form>
  </div>

  <script>
    (function () {
      const fileInput = document.getElementById('svg');
      const nameInput = document.getElementById('name');
      if (!fileInput || !nameInput) return;

      let lastAutoName = "";

      fileInput.addEventListener('change', function () {
        const file = this.files && this.files[0];
        if (!file) return;

        const fullName = file.name || "";
        const dot = fullName.lastIndexOf('.');
        const base = dot > 0 ? fullName.slice(0, dot) : fullName;

        // Only overwrite if the field is empty or matches the last auto-filled name
        if (nameInput.value.trim() === "" || nameInput.value === lastAutoName) {
          nameInput.value = base;
          lastAutoName = base;
        }
      });
    })();
  </script>
</body>
</html>
"#,
    )
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

    // lib.scad parameters:
    let mut lib_params = LibParams::default();

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
            _ => {
                lib_params
                    .set_from_field(&name, &text)
                    .map_err(|_| StatusCode::BAD_REQUEST)?;
            }
        }
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
        &lib_params,
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

    // ... read STL + build response as before ...
    let stl_bytes = tokio::fs::read(&stl_path).await.map_err(|err| {
        error!("Failed to read generated STL: {err}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let mut headers = HeaderMap::new();

    // Whatever you’re using as the base name:
    let safe_name = sanitize_filename_component(&lib_params.name);

    headers.insert(header::CONTENT_TYPE, HeaderValue::from_static("model/stl"));

    // Build the disposition string at runtime
    let disposition = format!("attachment; filename=\"{safe_name}.stl\"");

    // Convert it into a HeaderValue safely
    let disposition_value = HeaderValue::from_str(&disposition).map_err(|err| {
        error!("Invalid Content-Disposition header value: {err}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    headers.insert(header::CONTENT_DISPOSITION, disposition_value);

    Ok((headers, stl_bytes).into_response())
}

async fn shutdown_signal() {
    // Wait for either Ctrl+C or SIGTERM.
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

/// Replace any non header-safe / filesystem-safe characters with `_`.
/// Keeps only ASCII alphanumerics, `-`, and `_`.
fn sanitize_filename_component(raw: &str) -> String {
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

/// Build the OpenSCAD command-line args as a pure Vec<String> so tests can
/// validate ordering and values without spawning OpenSCAD.
fn build_openscad_args(
    fs: f32,
    fa: f32,
    fn_: i32,
    lib_params: &LibParams,
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

    // lib.scad params:
    args.push("-D".into());
    args.push(format!("NAME=\"{}\"", lib_params.name));
    args.push("-D".into());
    args.push(format!("MODE=\"{}\"", lib_params.mode));
    args.push("-D".into());
    args.push(format!("SHAPE=\"{}\"", lib_params.shape));
    args.push("-D".into());
    args.push(format!("SHAPE_ROT={}", lib_params.shape_rot));
    args.push("-D".into());
    args.push(format!("COASTER_D={}", lib_params.coaster_d));
    args.push("-D".into());
    args.push(format!("BASE_H={}", lib_params.base_h));
    args.push("-D".into());
    args.push(format!("INLAY_DH={}", lib_params.inlay_dh));
    args.push("-D".into());
    args.push(format!("MARGIN={}", lib_params.margin));
    args.push("-D".into());
    args.push(format!("CLEARANCE={}", lib_params.clearance));
    args.push("-D".into());
    args.push(format!("SEG={}", lib_params.seg));
    args.push("-D".into());
    args.push(format!(
        "INTERLOCK={}",
        if lib_params.interlock {
            "true"
        } else {
            "false"
        }
    ));
    args.push("-D".into());
    args.push(format!("EDGE_CLEAR={}", lib_params.edge_clear));
    args.push("-D".into());
    args.push(format!("MAG_D={}", lib_params.mag_d));
    args.push("-D".into());
    args.push(format!("MAG_H={}", lib_params.mag_h));
    args.push("-D".into());
    args.push(format!("MAG_CLEAR={}", lib_params.mag_clear));
    args.push("-D".into());
    args.push(format!("BOTTOM_SKIN={}", lib_params.bottom_skin));
    args.push("-D".into());
    args.push(format!("BOSS_CLEARANCE={}", lib_params.boss_clearance));
    args.push("-D".into());
    args.push(format!("BOSS_H={}", lib_params.boss_h));
    args.push("-D".into());
    args.push(format!("SPINNER_D={}", lib_params.spinner_d));
    args.push("-D".into());
    args.push(format!(
        "USE_SPINNER={}",
        if lib_params.use_spinner {
            "true"
        } else {
            "false"
        }
    ));

    // SVG path override:
    args.push("-D".into());
    args.push(format!("SVG_PATH=\"{}\"", svg_path.display()));

    // output + main .scad:
    args.push("-o".into());
    args.push(stl_path.to_string_lossy().to_string());
    args.push(input_scad_path.to_string_lossy().to_string());

    args
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn set_from_field_empty_text_is_noop() {
        let mut p = LibParams::default();
        let orig = p.clone();
        p.set_from_field("mode", "").unwrap();
        p.set_from_field("coaster_d", "").unwrap();
        assert_eq!(p.mode, orig.mode);
        assert_eq!(p.coaster_d, orig.coaster_d);
    }

    #[test]
    fn set_from_field_updates_string_fields() {
        let mut p = LibParams::default();
        p.set_from_field("name", "hello").unwrap();
        p.set_from_field("mode", "preview").unwrap();
        p.set_from_field("shape", "circle").unwrap();
        assert_eq!(p.name, "hello");
        assert_eq!(p.mode, "preview");
        assert_eq!(p.shape, "circle");
    }

    #[test]
    fn set_from_field_updates_numeric_fields() {
        let mut p = LibParams::default();
        p.set_from_field("shape_rot", "30.5").unwrap();
        p.set_from_field("coaster_d", "111.1").unwrap();
        p.set_from_field("base_h", "6.25").unwrap();
        p.set_from_field("seg", "123").unwrap();
        p.set_from_field("edge_clear", "9.9").unwrap();
        p.set_from_field("spinner_d", "17.0").unwrap();

        assert!((p.shape_rot - 30.5).abs() < 1e-6);
        assert!((p.coaster_d - 111.1).abs() < 1e-6);
        assert!((p.base_h - 6.25).abs() < 1e-6);
        assert_eq!(p.seg, 123);
        assert!((p.edge_clear - 9.9).abs() < 1e-6);
        assert!((p.spinner_d - 17.0).abs() < 1e-6);
    }

    #[test]
    fn set_from_field_updates_bool_fields() {
        let mut p = LibParams::default();
        p.set_from_field("interlock", "true").unwrap();
        p.set_from_field("use_spinner", "false").unwrap();
        assert_eq!(p.interlock, true);
        assert_eq!(p.use_spinner, false);
    }

    #[test]
    fn set_from_field_invalid_number_errors() {
        let mut p = LibParams::default();
        assert!(p.set_from_field("coaster_d", "nope").is_err());
        assert!(p.set_from_field("seg", "12.3").is_err());
    }

    #[test]
    fn set_from_field_invalid_bool_errors() {
        let mut p = LibParams::default();
        assert!(p.set_from_field("interlock", "perhaps").is_err());
        assert!(p.set_from_field("use_spinner", "2").is_err());
    }

    #[test]
    fn set_from_field_unknown_field_does_not_error() {
        let mut p = LibParams::default();
        let orig = p.clone();
        p.set_from_field("not_a_real_field", "123").unwrap();
        assert_eq!(p.name, orig.name);
        assert_eq!(p.seg, orig.seg);
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
        let mut p = LibParams::default();
        p.name = "My Logo".into();
        p.mode = "preview".into();
        p.shape = "circle".into();
        p.interlock = true;
        p.use_spinner = false;

        let svg = PathBuf::from("/tmp/input.svg");
        let stl = PathBuf::from("/tmp/output.stl");
        let main_scad = PathBuf::from("/app/input.scad");

        let args = build_openscad_args(0.25, 9.0, 123, &p, &svg, &stl, &main_scad);

        // head args
        assert_eq!(args[0], "--render");
        assert_eq!(args[1], "-D");
        assert_eq!(args[2], "fs=0.25");
        assert!(args.contains(&"fa=9".to_string()) || args.contains(&"fa=9.0".to_string()));
        assert!(args.contains(&"fn=123".to_string()));

        // some lib params
        assert!(args.contains(&"NAME=\"My Logo\"".to_string()));
        assert!(args.contains(&"MODE=\"preview\"".to_string()));
        assert!(args.contains(&"SHAPE=\"circle\"".to_string()));
        assert!(args.contains(&"INTERLOCK=true".to_string()));
        assert!(args.contains(&"USE_SPINNER=false".to_string()));

        // svg path override
        assert!(args.contains(&"SVG_PATH=\"/tmp/input.svg\"".to_string()));

        // tail order: -o stl main_scad
        let tail = &args[args.len() - 3..];
        assert_eq!(tail[0], "-o");
        assert_eq!(tail[1], "/tmp/output.stl");
        assert_eq!(tail[2], "/app/input.scad");
    }

    #[tokio::test]
    async fn index_returns_expected_html_bits() {
        let Html(body) = index().await;
        assert!(body.contains("<form action=\"/render\""));
        assert!(body.contains("name=\"svg\""));
        assert!(body.contains("name=\"name\""));
        assert!(body.contains("Generate STL from SVG"));
    }
}
