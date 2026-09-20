//! AI chat example page (feature `ai-chat`, default OFF): renders a transcript
//! fed by a [`ChatStreamSource`] via [`crate::services::streaming`].

pub mod view;

pub use view::AiResponseView;

use futures_util::Stream;
use gpui::App;

/// A participant in a chat exchange.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    User,
    Assistant,
}

/// One turn in a chat transcript.
#[derive(Clone, Debug)]
pub struct ChatTurn {
    pub role: Role,
    pub content: String,
}

impl ChatTurn {
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: Role::User,
            content: content.into(),
        }
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: Role::Assistant,
            content: content.into(),
        }
    }
}

/// Abstract source of an assistant token stream. `messages` is the full
/// transcript; the returned stream yields the reply as string chunks.
pub trait ChatStreamSource {
    /// Error produced by individual stream items; rendered via `Display`.
    type Error: std::fmt::Display + Send + 'static;
    /// The async stream type returned by [`Self::stream`].
    type Stream: Stream<Item = Result<String, Self::Error>> + Send + 'static;

    /// Begin streaming an assistant reply for the given transcript.
    fn stream(&self, messages: &[ChatTurn], cx: &App) -> Self::Stream;
}
