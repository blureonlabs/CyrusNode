//! Email sending. [`EmailSender`] trait + Resend implementation.
//!
//! Used by any feature that needs to send mail (drafting today, outreach
//! next). Features take `Arc<dyn EmailSender>` so transports can be swapped
//! for tests or future providers.

mod resend;
mod sender;

pub use resend::ResendClient;
pub use sender::{EmailError, EmailMessage, EmailSender, SentEmail};
