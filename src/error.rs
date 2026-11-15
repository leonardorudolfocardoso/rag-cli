use reqwest::StatusCode;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("retryable: {0}")]
    Retryable(String),
    #[error("permanent: {0}")]
    Permanent(String),
}

impl Error {
    pub(crate) fn from_status(status: StatusCode, body: String) -> Self {
        let msg = format!("{status}: {body}");
        if status == StatusCode::TOO_MANY_REQUESTS || status.is_server_error() {
            Error::Retryable(msg)
        } else {
            Error::Permanent(msg)
        }
    }

    pub fn is_retryable(&self) -> bool {
        matches!(self, Error::Retryable(_))
    }
}

impl From<reqwest::Error> for Error {
    fn from(e: reqwest::Error) -> Self {
        if e.is_timeout() || e.is_connect() || e.is_body() {
            Error::Retryable(e.to_string())
        } else {
            Error::Permanent(e.to_string())
        }
    }
}

#[cfg(test)]
mod test {
    use reqwest::StatusCode;

    use super::Error;

    #[test]
    fn from_status_classifies_retryable() {
        for status in [
            StatusCode::TOO_MANY_REQUESTS,
            StatusCode::INTERNAL_SERVER_ERROR,
            StatusCode::BAD_GATEWAY,
            StatusCode::SERVICE_UNAVAILABLE,
        ] {
            assert!(
                Error::from_status(status, String::new()).is_retryable(),
                "{status}"
            );
        }
    }

    #[test]
    fn from_status_classifies_permanent() {
        for status in [
            StatusCode::BAD_REQUEST,
            StatusCode::UNAUTHORIZED,
            StatusCode::FORBIDDEN,
            StatusCode::NOT_FOUND,
        ] {
            assert!(
                !Error::from_status(status, String::new()).is_retryable(),
                "{status}"
            );
        }
    }

    #[test]
    fn from_status_message_has_status_and_body() {
        let err = Error::from_status(StatusCode::BAD_REQUEST, "nope".into());
        assert_eq!(err.to_string(), "permanent: 400 Bad Request: nope");
    }
}
