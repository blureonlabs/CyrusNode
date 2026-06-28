//! On-disk queue under `out/dossiers/`. Each file is a `QueueItem` as JSON.
//! Approved/rejected files move into sibling dirs.

use std::path::PathBuf;

use async_trait::async_trait;

use crate::features::review::domain::{Decision, QueueItem, QueueRepoPort};

/// Filesystem-backed queue repository. Pending items live under
/// `<root>/dossiers/<company_id>.json`; transitions move the file into
/// `sent/`, `outbox/`, or `rejected/` sibling directories.
#[derive(Clone)]
pub struct FilesystemQueueRepo {
    root: PathBuf,
}

impl FilesystemQueueRepo {
    /// Construct a repo rooted at `root` (typically `out/`).
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    fn pending_dir(&self) -> PathBuf {
        self.root.join("dossiers")
    }
    fn sent_dir(&self) -> PathBuf {
        self.root.join("sent")
    }
    fn outbox_dir(&self) -> PathBuf {
        self.root.join("outbox")
    }
    fn rejected_dir(&self) -> PathBuf {
        self.root.join("rejected")
    }
}

#[async_trait]
impl QueueRepoPort for FilesystemQueueRepo {
    async fn pending(&self) -> anyhow::Result<Vec<QueueItem>> {
        let dir = self.pending_dir();
        if !dir.exists() {
            return Ok(Vec::new());
        }
        let mut items = Vec::new();
        let mut entries = tokio::fs::read_dir(&dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }
            let raw = tokio::fs::read_to_string(&path).await?;
            match serde_json::from_str::<QueueItem>(&raw) {
                Ok(item) => items.push(item),
                Err(e) => tracing::warn!(
                    path = %path.display(),
                    error = %e,
                    "queue: skipping malformed dossier"
                ),
            }
        }
        items.sort_by_key(|i| i.url.clone());
        Ok(items)
    }

    async fn record_decision(&self, item: &QueueItem, decision: Decision) -> anyhow::Result<()> {
        let from = self.pending_dir().join(format!("{}.json", item.company_id));
        let to_dir = match decision {
            Decision::ApproveAndSend => self.sent_dir(),
            Decision::ApproveToOutbox => self.outbox_dir(),
            Decision::Reject => self.rejected_dir(),
            Decision::Skip | Decision::Edit | Decision::Quit => return Ok(()),
        };
        tokio::fs::create_dir_all(&to_dir).await?;
        let to = to_dir.join(format!("{}.json", item.company_id));
        if from.exists() {
            tokio::fs::rename(&from, &to).await?;
        }
        Ok(())
    }

    async fn save_edited(&self, item: &QueueItem) -> anyhow::Result<()> {
        let path = self.pending_dir().join(format!("{}.json", item.company_id));
        let json = serde_json::to_string_pretty(item)?;
        tokio::fs::write(&path, json).await?;
        Ok(())
    }
}
