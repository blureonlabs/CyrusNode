//! Interactive terminal prompter using stdin. Sufficient for V1.

use std::io::Write;

use async_trait::async_trait;

use crate::features::review::domain::{Decision, OperatorPrompt, QueueItem};

/// Reads single-character decisions from stdin and shells out to `$EDITOR`
/// when the operator chooses [`Decision::Edit`].
pub struct StdinPrompter;

#[async_trait]
impl OperatorPrompt for StdinPrompter {
    async fn ask(&self, item: &QueueItem) -> anyhow::Result<Decision> {
        println!();
        println!("══════════════════════════════════════════════════════════════");
        println!("URL       : {}", item.url);
        println!("Industry  : {}", item.industry.as_deref().unwrap_or("-"));
        println!("Summary   : {}", item.summary_line);
        println!(
            "Recipient : {}",
            item.recipient_email
                .as_deref()
                .unwrap_or("(none — will go to outbox)")
        );
        println!("──────────────────────────────────────────────────────────────");
        println!("Subject: {}", item.email_subject);
        println!();
        println!("{}", item.email_body);
        println!();
        println!("Anchors used: {}", item.email_anchors.join(", "));
        println!("──────────────────────────────────────────────────────────────");
        print!("[a]pprove  [e]dit  [r]eject  [s]kip  [q]uit: ");
        std::io::stdout().flush().ok();

        // Read on a blocking task so the async runtime is happy.
        let s = tokio::task::spawn_blocking(move || -> std::io::Result<String> {
            let mut line = String::new();
            std::io::stdin().read_line(&mut line)?;
            Ok(line)
        })
        .await??;

        let c = s.trim().chars().next().unwrap_or('s');
        Ok(match c {
            'a' | 'A' => {
                if item.recipient_email.is_some() {
                    Decision::ApproveAndSend
                } else {
                    Decision::ApproveToOutbox
                }
            }
            'e' | 'E' => Decision::Edit,
            'r' | 'R' => Decision::Reject,
            'q' | 'Q' => Decision::Quit,
            _ => Decision::Skip,
        })
    }

    async fn request_recipient(&self, item: &QueueItem) -> anyhow::Result<Option<String>> {
        print!(
            "No recipient on {}. Enter email (blank → outbox): ",
            item.url
        );
        std::io::stdout().flush().ok();
        let s = tokio::task::spawn_blocking(move || -> std::io::Result<String> {
            let mut line = String::new();
            std::io::stdin().read_line(&mut line)?;
            Ok(line)
        })
        .await??;
        let trimmed = s.trim();
        if trimmed.is_empty() {
            Ok(None)
        } else if trimmed.contains('@') {
            Ok(Some(trimmed.to_string()))
        } else {
            println!("(not an email — sending to outbox)");
            Ok(None)
        }
    }

    async fn edit_body(&self, current: &str) -> anyhow::Result<String> {
        // Write current to a temp file, spawn $EDITOR (default `vi`), read back.
        let tmp = std::env::temp_dir().join(format!("apollo-edit-{}.md", uuid::Uuid::new_v4()));
        tokio::fs::write(&tmp, current).await?;
        let editor = std::env::var("EDITOR").unwrap_or_else(|_| "vi".to_string());
        let path_for_status = tmp.clone();
        let path_for_read = tmp.clone();
        let status = tokio::task::spawn_blocking(move || {
            std::process::Command::new(editor)
                .arg(&path_for_status)
                .status()
        })
        .await??;
        if !status.success() {
            anyhow::bail!("editor exited non-zero");
        }
        let edited = tokio::fs::read_to_string(&path_for_read).await?;
        let _ = tokio::fs::remove_file(&tmp).await;
        Ok(edited)
    }
}
