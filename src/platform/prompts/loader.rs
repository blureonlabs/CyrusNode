//! Hot-reloading prompt loader.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::Result;
use tokio::sync::RwLock;

use super::frontmatter::split;
use super::prompt::Prompt;

/// Hot-reloading prompt loader.
///
/// On construction (via [`PromptLoader::load`]) it scans the directory once.
/// Call [`PromptLoader::watch`] to spawn a background task that re-reads
/// changed files via the `notify` file-watcher crate.
///
/// Cheap to clone — internal state is `Arc<RwLock<...>>`.
#[derive(Clone)]
pub struct PromptLoader {
    root: PathBuf,
    cache: Arc<RwLock<HashMap<String, Prompt>>>,
}

impl PromptLoader {
    /// Load all `*.md` files under `root` (non-recursive — Apollo's prompts
    /// live as a flat directory).
    ///
    /// Files whose stem begins with `_` (e.g. `_overview.md`) or equals
    /// `README` (any case) are skipped. Files that fail to parse are logged
    /// at WARN and skipped — they do not abort the load.
    ///
    /// # Failure modes
    /// Returns an error if the directory cannot be read.
    pub async fn load(root: impl AsRef<Path>) -> Result<Self> {
        let root = root.as_ref().to_path_buf();
        let loader = Self {
            root: root.clone(),
            cache: Arc::new(RwLock::new(HashMap::new())),
        };
        loader.scan().await?;
        Ok(loader)
    }

    async fn scan(&self) -> Result<()> {
        let mut next = HashMap::new();
        let mut entries = tokio::fs::read_dir(&self.root).await?;
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) != Some("md") {
                continue;
            }
            // Skip `_overview.md`, README, and other underscore-prefixed files.
            let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
            if stem.starts_with('_') || stem.eq_ignore_ascii_case("README") {
                continue;
            }
            let content = match tokio::fs::read_to_string(&path).await {
                Ok(c) => c,
                Err(e) => {
                    tracing::warn!(path = %path.display(), error = %e, "prompt read failed; skipping");
                    continue;
                }
            };
            match split(&content) {
                Ok((fm, body)) => {
                    let p = Prompt::new(fm.clone(), body.to_string());
                    next.insert(fm.name.clone(), p);
                }
                Err(e) => {
                    tracing::warn!(path = %path.display(), error = %e, "prompt parse failed; skipping");
                }
            }
        }
        let mut cache = self.cache.write().await;
        *cache = next;
        tracing::info!(count = cache.len(), "prompts loaded");
        Ok(())
    }

    /// Look up a prompt by `name` (the frontmatter `name:` field).
    ///
    /// Returns `None` if no prompt with that name is currently loaded.
    pub async fn get(&self, name: &str) -> Option<Prompt> {
        self.cache.read().await.get(name).cloned()
    }

    /// Return the names of every prompt currently in the cache.
    pub async fn names(&self) -> Vec<String> {
        self.cache.read().await.keys().cloned().collect()
    }

    /// Spawn a background task that watches `root` for changes and re-scans
    /// on every file write. Returns immediately.
    ///
    /// The watcher is moved into the spawned task and lives as long as the
    /// task does — i.e. until the event channel closes.
    ///
    /// # Failure modes
    /// Returns an error if the file watcher cannot be created or fails to
    /// register the directory.
    pub fn watch(&self) -> Result<()> {
        use notify::{RecommendedWatcher, RecursiveMode, Watcher};

        let loader = self.clone();

        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let mut watcher: RecommendedWatcher = RecommendedWatcher::new(
            move |res: Result<notify::Event, notify::Error>| {
                if res.is_ok() {
                    let _ = tx.send(());
                }
            },
            notify::Config::default(),
        )?;
        watcher.watch(&self.root, RecursiveMode::NonRecursive)?;

        tokio::spawn(async move {
            // Keep watcher alive for the lifetime of the task.
            let _w = watcher;
            while rx.recv().await.is_some() {
                if let Err(e) = loader.scan().await {
                    tracing::warn!(error = %e, "prompt rescan failed");
                }
            }
        });
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn loads_real_apollo_prompts_directory() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("apollo/04-prompts");
        let loader = PromptLoader::load(&root).await.expect("loads");
        let bs = loader
            .get("business-summary")
            .await
            .expect("business-summary present");
        assert_eq!(bs.frontmatter.version, 1);
        assert_eq!(bs.frontmatter.max_output_tokens, 4000);

        let names = loader.names().await;
        // `_overview.md` must NOT appear as a prompt.
        assert!(!names.iter().any(|n| n == "_overview"));
        assert!(names.iter().any(|n| n == "business-summary"));
    }

    #[tokio::test]
    async fn missing_prompt_returns_none() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("apollo/04-prompts");
        let loader = PromptLoader::load(&root).await.expect("loads");
        assert!(loader.get("does-not-exist").await.is_none());
    }
}
