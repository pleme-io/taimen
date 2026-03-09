use thiserror::Error;

/// Alias for `std::result::Result<T, TaimenError>`.
pub type Result<T> = std::result::Result<T, TaimenError>;

/// Errors that can occur within Taimen.
#[derive(Debug, Error)]
pub enum TaimenError {
    /// A room-related error (creation, lookup, state transition).
    #[error("room error: {0}")]
    Room(String),

    /// A signaling-protocol error.
    #[error("signal error: {0}")]
    Signal(String),

    /// A media-pipeline error (track negotiation, codec mismatch).
    #[error("media error: {0}")]
    Media(String),

    /// An authentication or authorization error.
    #[error("auth error: {0}")]
    Auth(String),

    /// A WebRTC transport error.
    #[error("webrtc error: {0}")]
    WebRTC(String),

    /// The room has reached its maximum participant capacity.
    #[error("capacity error: {0}")]
    Capacity(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn room_error_displays() {
        let err = TaimenError::Room("not found".into());
        assert_eq!(err.to_string(), "room error: not found");
    }

    #[test]
    fn signal_error_displays() {
        let err = TaimenError::Signal("invalid offer".into());
        assert_eq!(err.to_string(), "signal error: invalid offer");
    }

    #[test]
    fn media_error_displays() {
        let err = TaimenError::Media("codec unsupported".into());
        assert_eq!(err.to_string(), "media error: codec unsupported");
    }

    #[test]
    fn auth_error_displays() {
        let err = TaimenError::Auth("unauthorized".into());
        assert_eq!(err.to_string(), "auth error: unauthorized");
    }

    #[test]
    fn webrtc_error_displays() {
        let err = TaimenError::WebRTC("ice failed".into());
        assert_eq!(err.to_string(), "webrtc error: ice failed");
    }

    #[test]
    fn capacity_error_displays() {
        let err = TaimenError::Capacity("room full".into());
        assert_eq!(err.to_string(), "capacity error: room full");
    }

    #[test]
    fn result_alias_works() {
        let ok: Result<u32> = Ok(42);
        assert_eq!(ok.unwrap(), 42);

        let err: Result<u32> = Err(TaimenError::Room("gone".into()));
        assert!(err.is_err());
    }
}
