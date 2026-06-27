//! On-disk storage for fetched HTML.
//!
//! V1 writes raw HTML to a per-correlation directory under the configured
//! `output_root`. V6 will move this to object storage (see crawler.md
//! "Outputs.html_path" — `s3://...` is the planned form).

use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};
use url::Url;

/// Save HTML body keyed by `sha256(url)`. Returns the on-disk path.
///
/// File name is the first 16 hex chars of the SHA-256 of the URL, which is
/// short enough to be readable and collision-free for our scale.
pub async fn save_html(out_dir: &Path, url: &Url, html: &str) -> std::io::Result<PathBuf> {
    let mut hasher = Sha256::new();
    hasher.update(url.as_str().as_bytes());
    let hash = hex::encode(hasher.finalize());
    let name = &hash[..16];
    let path = out_dir.join(format!("{}.html", name));
    tokio::fs::write(&path, html.as_bytes()).await?;
    Ok(path)
}
