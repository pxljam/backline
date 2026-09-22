//! Tech rider PDF export through **Typst** (§15).
//!
//! Deterministic output, no browser in the loop: this is a structured document,
//! not a poster. The visuals render engine (Remotion) has no business here.

use crate::error::{AppError, AppResult};
use serde_json::Value;
use std::path::PathBuf;

fn template_dir() -> PathBuf {
    std::env::var("TYPST_TEMPLATE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/app/pdf"))
}

fn typst_bin() -> String {
    std::env::var("TYPST_BIN").unwrap_or_else(|_| "typst".into())
}

pub fn available() -> bool {
    std::process::Command::new(typst_bin())
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Compiles `template` with `data` serialised to `data.json`, returns the PDF.
pub async fn compile(template: &str, data: &Value) -> AppResult<Vec<u8>> {
    let dir = template_dir();
    let template_path = dir.join(template);
    if !template_path.exists() {
        return Err(AppError::Internal(anyhow::anyhow!(
            "modele Typst introuvable : {}",
            template_path.display()
        )));
    }

    let work = std::env::temp_dir().join(format!("backline-typst-{}", uuid::Uuid::new_v4()));
    tokio::fs::create_dir_all(&work)
        .await
        .map_err(|e| AppError::Internal(e.into()))?;

    let result = compile_in(&work, &template_path, data).await;
    let _ = tokio::fs::remove_dir_all(&work).await;
    result
}

async fn compile_in(
    work: &std::path::Path,
    template_path: &std::path::Path,
    data: &Value,
) -> AppResult<Vec<u8>> {
    tokio::fs::write(
        work.join("data.json"),
        serde_json::to_vec_pretty(data).unwrap(),
    )
    .await
    .map_err(|e| AppError::Internal(e.into()))?;
    let main = work.join("main.typ");
    tokio::fs::copy(template_path, &main)
        .await
        .map_err(|e| AppError::Internal(e.into()))?;

    let out = work.join("out.pdf");
    let output = tokio::process::Command::new(typst_bin())
        .arg("compile")
        .arg("--root")
        .arg(work)
        .arg(&main)
        .arg(&out)
        .output()
        .await
        .map_err(|e| {
            AppError::Internal(anyhow::anyhow!(
                "typst introuvable ({e}). Installer le binaire ou definir TYPST_BIN."
            ))
        })?;

    if !output.status.success() {
        return Err(AppError::Internal(anyhow::anyhow!(
            "typst a echoue: {}",
            String::from_utf8_lossy(&output.stderr)
        )));
    }

    tokio::fs::read(&out)
        .await
        .map_err(|e| AppError::Internal(e.into()))
}
