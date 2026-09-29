//! Errors with a stable kind, so scripts and agents can react to what went
//! wrong from the exit code alone, or from the JSON error object on stderr.

use std::fmt;

use serde::Serialize;

/// What went wrong. Each kind has its own exit code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    /// Anything not covered below.
    Other,
    /// Bad arguments or input, including unparseable dates.
    Usage,
    /// The task, project, label or user does not exist.
    NotFound,
    /// Not logged in, or the token lacks permission.
    Auth,
    /// The task changed since it was read, or is claimed by someone else.
    Conflict,
    /// The server rejected the request.
    Api,
    /// The server could not be reached, or sent something unreadable.
    Network,
}

impl Kind {
    pub fn exit_code(self) -> i32 {
        match self {
            Kind::Other => 1,
            Kind::Usage => 2,
            Kind::NotFound => 3,
            Kind::Auth => 4,
            Kind::Conflict => 5,
            Kind::Api => 6,
            Kind::Network => 7,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct VkError {
    pub kind: Kind,
    pub message: String,
}

pub type Result<T, E = VkError> = std::result::Result<T, E>;

impl VkError {
    pub fn new(kind: Kind, message: impl Into<String>) -> Self {
        VkError {
            kind,
            message: message.into(),
        }
    }

    pub fn usage(message: impl Into<String>) -> Self {
        Self::new(Kind::Usage, message)
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new(Kind::NotFound, message)
    }

    pub fn other(message: impl Into<String>) -> Self {
        Self::new(Kind::Other, message)
    }

    /// Print to stderr: `{"error": {"kind": .., "message": ..}}` in JSON mode,
    /// a red line otherwise.
    pub fn report(&self, json: bool) {
        if json {
            let body = serde_json::json!({ "error": self });
            eprintln!("{body}");
        } else if std::io::IsTerminal::is_terminal(&std::io::stderr()) {
            eprintln!("\x1b[31m{}\x1b[0m", self.message);
        } else {
            eprintln!("{}", self.message);
        }
    }
}

impl fmt::Display for VkError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for VkError {}

/// Vikunja error bodies look like `{"code": 4013, "message": "..."}`.
fn server_message(content: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(content).ok()?;
    value.get("message")?.as_str().map(str::to_string)
}

impl<T> From<vikunjars::apis::Error<T>> for VkError {
    fn from(err: vikunjars::apis::Error<T>) -> Self {
        use vikunjars::apis::Error;

        match err {
            Error::ResponseError(resp) => {
                let status = resp.status.as_u16();
                let detail = server_message(&resp.content).unwrap_or_else(|| resp.content.clone());
                let kind = match status {
                    401 | 403 => Kind::Auth,
                    404 => Kind::NotFound,
                    409 | 412 => Kind::Conflict,
                    400 | 422 => Kind::Usage,
                    _ => Kind::Api,
                };
                let message = if detail.is_empty() {
                    format!("Server returned {status}")
                } else {
                    format!("Server returned {status}: {detail}")
                };
                VkError::new(kind, message)
            }
            Error::Reqwest(e) => VkError::new(Kind::Network, format!("Request failed: {e}")),
            Error::Serde(e) => VkError::new(
                Kind::Network,
                format!("Could not read the server's response: {e}"),
            ),
            Error::Io(e) => VkError::new(Kind::Network, format!("I/O error: {e}")),
        }
    }
}

impl From<std::io::Error> for VkError {
    fn from(err: std::io::Error) -> Self {
        VkError::other(err.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vikunjars::apis::{Error, ResponseContent};

    fn response(status: u16, content: &str) -> Error<()> {
        Error::ResponseError(ResponseContent {
            status: reqwest::StatusCode::from_u16(status).unwrap(),
            content: content.to_string(),
            entity: None,
        })
    }

    #[test]
    fn test_status_codes_map_to_kinds() {
        let kind = |s| VkError::from(response(s, "")).kind;
        assert_eq!(kind(401), Kind::Auth);
        assert_eq!(kind(403), Kind::Auth);
        assert_eq!(kind(404), Kind::NotFound);
        assert_eq!(kind(412), Kind::Conflict);
        assert_eq!(kind(400), Kind::Usage);
        assert_eq!(kind(500), Kind::Api);
    }

    #[test]
    fn test_server_message_is_extracted() {
        let err = VkError::from(response(
            404,
            r#"{"code":2001,"message":"The task does not exist."}"#,
        ));
        assert_eq!(err.message, "Server returned 404: The task does not exist.");

        let err = VkError::from(response(502, "Bad Gateway"));
        assert_eq!(err.message, "Server returned 502: Bad Gateway");
    }

    #[test]
    fn test_exit_codes_are_distinct() {
        let kinds = [
            Kind::Other,
            Kind::Usage,
            Kind::NotFound,
            Kind::Auth,
            Kind::Conflict,
            Kind::Api,
            Kind::Network,
        ];
        let mut codes: Vec<i32> = kinds.iter().map(|k| k.exit_code()).collect();
        codes.dedup();
        assert_eq!(codes.len(), kinds.len());
        assert!(codes.iter().all(|&c| c != 0 && c != 101));
    }

    #[test]
    fn test_json_error_shape() {
        let err = VkError::new(Kind::Conflict, "changed");
        let body = serde_json::json!({ "error": err });
        assert_eq!(body["error"]["kind"], "conflict");
        assert_eq!(body["error"]["message"], "changed");
    }
}
