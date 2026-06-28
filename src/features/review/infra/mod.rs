//! Adapters that implement the review feature's domain ports.

mod email_send_adapter;
mod filesystem_queue;
mod stdin_prompter;

pub use email_send_adapter::{DryRunSendAdapter, EmailSendAdapter};
pub use filesystem_queue::FilesystemQueueRepo;
pub use stdin_prompter::StdinPrompter;
