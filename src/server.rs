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
/// Simple HTML form for manual testing.
async fn index() -> Html<&'static str> {
    Html(
        r#"<!DOCTYPE html>
<html>
<head>
  <meta charset="utf-8">
  <title>OpenSCAD STL Generator</title>
</head>
<body>
  <h1>Generate STL from SVG</h1>
  <form action="/render" method="post" enctype="multipart/form-data">
    <div>
      <label>SVG file:
        <input type="file" name="svg" accept=".svg" required>
      </label>
    </div>
    <div>
      <label>fs:
        <input type="number" step="0.01" name="fs" value="0.1">
      </label>
    </div>
    <div>
      <label>fa:
        <input type="number" step="1" name="fa" value="5">
      </label>
    </div>
    <div>
      <label>fn:
        <input type="number" step="1" name="fn" value="200">
      </label>
    </div>
    <button type="submit">Generate STL</button>
  </form>
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
    let mut fs: f32 = 0.1;
    let mut fa: f32 = 5.0;
    let mut fn_: i32 = 200;

    // Parse multipart fields
    while let Some(field) = multipart.next_field().await.map_err(|err| {
        error!("Failed to read multipart field: {err}");
        StatusCode::BAD_REQUEST
    })? {
        let name = field.name().unwrap_or("").to_string();
        debug!("Received multipart field: {name}");

        match name.as_str() {
            "svg" => {
                let bytes = field.bytes().await.map_err(|err| {
                    error!("Failed to read svg field: {err}");
                    StatusCode::BAD_REQUEST
                })?;
                svg_bytes = Some(bytes);
            }
            "fs" => {
                let text = field.text().await.map_err(|_| StatusCode::BAD_REQUEST)?;
                if !text.is_empty() {
                    fs = text.parse().map_err(|_| StatusCode::BAD_REQUEST)?;
                }
            }
            "fa" => {
                let text = field.text().await.map_err(|_| StatusCode::BAD_REQUEST)?;
                if !text.is_empty() {
                    fa = text.parse().map_err(|_| StatusCode::BAD_REQUEST)?;
                }
            }
            "fn" => {
                let text = field.text().await.map_err(|_| StatusCode::BAD_REQUEST)?;
                if !text.is_empty() {
                    fn_ = text.parse().map_err(|_| StatusCode::BAD_REQUEST)?;
                }
            }
            _ => {
                // Ignore unknown fields for now
            }
        }
    }

    let svg_bytes = svg_bytes.ok_or(StatusCode::BAD_REQUEST)?;

    // Create a temp directory for this request
    let tmpdir = tempdir().map_err(|err| {
        error!("Failed to create temp dir: {err}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    let svg_path = tmpdir.path().join("input.svg");
    let stl_path = tmpdir.path().join("output.stl");

    // Write SVG file
    tokio::fs::write(&svg_path, &svg_bytes)
        .await
        .map_err(|err| {
            error!("Failed to write SVG to disk: {err}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    // Build the openscad command
    let fs_arg = format!("fs={fs}");
    let fa_arg = format!("fa={fa}");
    let fn_arg = format!("fn={fn_}");

    // NOTE: tile.scad can reference 'input.svg' in the same directory,
    // or you can extend this to also pass svg_path via -D svg_path="...".
    info!("Running openscad to generate STL...");
    let status = Command::new("openscad")
        .arg("--render")
        .arg("-D")
        .arg(fs_arg)
        .arg("-D")
        .arg(fa_arg)
        .arg("-D")
        .arg(fn_arg)
        .arg("-o")
        .arg(&stl_path)
        .arg(&state.input_scad_path)
        .status()
        .await
        .map_err(|err| {
            error!("Failed to spawn openscad: {err}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    if !status.success() {
        error!("openscad exited with non-zero status: {status}");
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    }

    // Read the generated STL
    let stl_bytes = tokio::fs::read(&stl_path).await.map_err(|err| {
        error!("Failed to read generated STL: {err}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // Build response with download headers
    let mut headers = HeaderMap::new();
    headers.insert(header::CONTENT_TYPE, HeaderValue::from_static("model/stl"));
    headers.insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_static("attachment; filename=\"tile.stl\""),
    );

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
