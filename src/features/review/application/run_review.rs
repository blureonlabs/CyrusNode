//! Use case: walk the operator through pending dossiers.

use std::sync::Arc;

use crate::features::review::domain::{Decision, OperatorPrompt, QueueRepoPort, SendPort};

/// Orchestrates the review loop: pull pending items, ask the operator,
/// dispatch or persist according to the chosen [`Decision`].
pub struct RunReview {
    queue: Arc<dyn QueueRepoPort>,
    prompter: Arc<dyn OperatorPrompt>,
    sender: Arc<dyn SendPort>,
    /// e.g. `"Hari <hari@apollo.dev>"`.
    from_address: String,
}

/// Tally of what happened during a review session.
pub struct ReviewSummary {
    /// Number of items shown to the operator.
    pub reviewed: u32,
    /// Approved and successfully dispatched through the sender.
    pub approved_sent: u32,
    /// Approved without a recipient — written to the outbox dir for manual send.
    pub approved_outbox: u32,
    /// Rejected by the operator.
    pub rejected: u32,
    /// Skipped (left in the pending dir).
    pub skipped: u32,
}

impl RunReview {
    /// Construct the use case from its ports.
    pub fn new(
        queue: Arc<dyn QueueRepoPort>,
        prompter: Arc<dyn OperatorPrompt>,
        sender: Arc<dyn SendPort>,
        from_address: String,
    ) -> Self {
        Self {
            queue,
            prompter,
            sender,
            from_address,
        }
    }

    /// Run the review loop end-to-end. Returns a [`ReviewSummary`] tallying
    /// every decision the operator made.
    pub async fn run(&self) -> anyhow::Result<ReviewSummary> {
        let mut items = self.queue.pending().await?;
        let mut summary = ReviewSummary {
            reviewed: 0,
            approved_sent: 0,
            approved_outbox: 0,
            rejected: 0,
            skipped: 0,
        };
        for item in items.iter_mut() {
            summary.reviewed += 1;
            let decision = self.prompter.ask(item).await?;
            match decision {
                Decision::ApproveAndSend => {
                    let to = match item.recipient_email.clone() {
                        Some(r) => r,
                        None => {
                            // Ask the operator inline. Persist if they provide one.
                            match self.prompter.request_recipient(item).await? {
                                Some(addr) => {
                                    item.recipient_email = Some(addr.clone());
                                    self.queue.save_edited(item).await?;
                                    addr
                                }
                                None => {
                                    self.queue
                                        .record_decision(item, Decision::ApproveToOutbox)
                                        .await?;
                                    summary.approved_outbox += 1;
                                    continue;
                                }
                            }
                        }
                    };
                    let _ = self
                        .sender
                        .send(
                            &self.from_address,
                            &to,
                            &item.email_subject,
                            &item.email_body,
                        )
                        .await?;
                    self.queue
                        .record_decision(item, Decision::ApproveAndSend)
                        .await?;
                    summary.approved_sent += 1;
                }
                Decision::ApproveToOutbox => {
                    self.queue
                        .record_decision(item, Decision::ApproveToOutbox)
                        .await?;
                    summary.approved_outbox += 1;
                }
                Decision::Edit => {
                    let new_body = self.prompter.edit_body(&item.email_body).await?;
                    item.email_body = new_body;
                    self.queue.save_edited(item).await?;
                    // After edit, ask again — same loop iteration. Simplest:
                    // re-prompt once more.
                    let again = self.prompter.ask(item).await?;
                    match again {
                        Decision::ApproveAndSend => {
                            if let Some(to) = item.recipient_email.clone() {
                                let _ = self
                                    .sender
                                    .send(
                                        &self.from_address,
                                        &to,
                                        &item.email_subject,
                                        &item.email_body,
                                    )
                                    .await?;
                                self.queue
                                    .record_decision(item, Decision::ApproveAndSend)
                                    .await?;
                                summary.approved_sent += 1;
                            } else {
                                self.queue
                                    .record_decision(item, Decision::ApproveToOutbox)
                                    .await?;
                                summary.approved_outbox += 1;
                            }
                        }
                        Decision::Reject => {
                            self.queue.record_decision(item, Decision::Reject).await?;
                            summary.rejected += 1;
                        }
                        _ => {
                            summary.skipped += 1;
                        }
                    }
                }
                Decision::Reject => {
                    self.queue.record_decision(item, Decision::Reject).await?;
                    summary.rejected += 1;
                }
                Decision::Skip => {
                    summary.skipped += 1;
                }
                Decision::Quit => break,
            }
        }
        Ok(summary)
    }
}
