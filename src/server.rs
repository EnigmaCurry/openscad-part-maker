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

    // Build openscad command with all -D params
    let mut cmd = Command::new("openscad");
    cmd.arg("--render")
        .arg("-D")
        .arg(format!("fs={fs}"))
        .arg("-D")
        .arg(format!("fa={fa}"))
        .arg("-D")
        .arg(format!("fn={fn_}"))
        // lib.scad params:
        .arg("-D")
        .arg(format!("NAME=\"{}\"", lib_params.name))
        .arg("-D")
        .arg(format!("MODE=\"{}\"", lib_params.mode))
        .arg("-D")
        .arg(format!("SHAPE=\"{}\"", lib_params.shape))
        .arg("-D")
        .arg(format!("SHAPE_ROT={}", lib_params.shape_rot))
        .arg("-D")
        .arg(format!("COASTER_D={}", lib_params.coaster_d))
        .arg("-D")
        .arg(format!("BASE_H={}", lib_params.base_h))
        .arg("-D")
        .arg(format!("INLAY_DH={}", lib_params.inlay_dh))
        .arg("-D")
        .arg(format!("MARGIN={}", lib_params.margin))
        .arg("-D")
        .arg(format!("CLEARANCE={}", lib_params.clearance))
        .arg("-D")
        .arg(format!("SEG={}", lib_params.seg))
        .arg("-D")
        .arg(format!(
            "INTERLOCK={}",
            if lib_params.interlock {
                "true"
            } else {
                "false"
            }
        ))
        .arg("-D")
        .arg(format!("EDGE_CLEAR={}", lib_params.edge_clear))
        .arg("-D")
        .arg(format!("MAG_D={}", lib_params.mag_d))
        .arg("-D")
        .arg(format!("MAG_H={}", lib_params.mag_h))
        .arg("-D")
        .arg(format!("MAG_CLEAR={}", lib_params.mag_clear))
        .arg("-D")
        .arg(format!("BOTTOM_SKIN={}", lib_params.bottom_skin))
        .arg("-D")
        .arg(format!("BOSS_CLEARANCE={}", lib_params.boss_clearance))
        .arg("-D")
        .arg(format!("BOSS_H={}", lib_params.boss_h))
        .arg("-D")
        .arg(format!("SPINNER_D={}", lib_params.spinner_d))
        .arg("-D")
        .arg(format!(
            "USE_SPINNER={}",
            if lib_params.use_spinner {
                "true"
            } else {
                "false"
            }
        ))
        // SVG path: we override SVG_PATH in lib.scad
        .arg("-D")
        .arg(format!("SVG_PATH=\"{}\"", svg_path.display()))
        // output and main .scad file
        .arg("-o")
        .arg(&stl_path)
        .arg(&state.input_scad_path);

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
    let raw_name = lib_params.name.clone();

    // Optional: sanitize to something header-safe / filesystem-safe
    let safe_name: String = raw_name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();

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
