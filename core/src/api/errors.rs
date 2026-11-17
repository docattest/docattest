//! The two kinds of failure.

use std::fmt;

/// Why a command failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// A problem with the project, the request or the machine: a bad config, a page that
    /// cannot be read, a fetch or a write that failed. The message is complete (it starts
    /// with `docattest: `, or with the adapter's own prefix); the command line prints it
    /// without a stack and exits 1.
    User(String),
    /// A broken invariant: a bug in docattest or in the adapter, never the user's mistake.
    /// The command line asks for it to be reported and exits 2; `detail` follows the message.
    Internal { message: String, detail: String },
}

/// What a user error's message starts with, unless an adapter gave its own.
const PREFIX: &str = "docattest: ";

impl Error {
    /// A user error, prefixed `docattest: `.
    pub fn user(message: impl fmt::Display) -> Error {
        Error::User(format!("{PREFIX}{message}"))
    }

    /// A user error whose message already carries its prefix (an adapter's).
    pub fn user_verbatim(message: impl Into<String>) -> Error {
        Error::User(message.into())
    }

    /// An internal error.
    pub fn internal(message: impl fmt::Display) -> Error {
        Error::Internal { message: message.to_string(), detail: String::new() }
    }

    /// This error told inside `context`, such as the command or member that met it:
    /// `docattest: {context}: {message}` for a user error; an internal one stays as it is.
    pub fn within(self, context: impl fmt::Display) -> Error {
        match self.into_user_message() {
            Ok(message) => Error::user(format!("{context}: {message}")),
            Err(internal) => internal,
        }
    }

    /// A user error's message without the `docattest: ` it starts with, to be told inside
    /// another message; an internal error comes back as the error, since it is a bug to
    /// report, never a reason to tell the user.
    pub fn into_user_message(self) -> std::result::Result<String, Error> {
        match self {
            Error::User(message) => Ok(unprefixed(&message).to_string()),
            internal => Err(internal),
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::User(m) => f.write_str(m),
            Error::Internal { message, detail } if detail.is_empty() => f.write_str(message),
            Error::Internal { message, detail } => write!(f, "{message}\n{detail}"),
        }
    }
}

pub type Result<T> = std::result::Result<T, Error>;

/// A user error's message without the `docattest: ` it starts with, to be told inside
/// another message.
pub fn unprefixed(message: &str) -> &str {
    message.strip_prefix(PREFIX).unwrap_or(message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_problem_told_within_names_what_met_it() {
        let failed = Error::user("surface \"default\" failed to load: Error: boom");
        assert_eq!(
            failed.within("inventory").to_string(),
            "docattest: inventory: surface \"default\" failed to load: Error: boom"
        );
        assert_eq!(Error::internal("x").within("audit"), Error::internal("x"));
    }
}
