use super::{StoredTransient, TransientData, TransientRequest, TransientStore};
use axum::response::Response;
use std::{error::Error, fmt};

const KEY: &str = "inertia.transient";

/// Optional adapter backed by `tower-sessions`.
#[derive(Clone, Default)]
pub struct TowerSessionTransient;
impl TowerSessionTransient {
    /// Creates the adapter.
    pub fn new() -> Self {
        Self
    }
}

#[derive(Debug)]
pub struct TowerSessionTransientError(String);
impl fmt::Display for TowerSessionTransientError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
impl Error for TowerSessionTransientError {}

impl TransientStore for TowerSessionTransient {
    type Error = TowerSessionTransientError;
    async fn load(&self, request: TransientRequest<'_>) -> Result<TransientData, Self::Error> {
        let session = request.tower_session().ok_or_else(|| TowerSessionTransientError("TowerSessionTransient requires tower_sessions::Session in request extensions; install SessionManagerLayer outside the Inertia layer".to_owned()))?;
        let stored = session
            .remove::<StoredTransient>(KEY)
            .await
            .map_err(|error| TowerSessionTransientError(error.to_string()))?
            .unwrap_or_default();
        Ok(TransientData::loaded(stored, "tower-session").with_session(session.clone()))
    }
    async fn commit(
        &self,
        _response: &mut Response,
        data: TransientData,
    ) -> Result<(), Self::Error> {
        let (stored, session) = data.into_session_parts();
        let session = session.ok_or_else(|| {
            TowerSessionTransientError(
                "TowerSessionTransient lost its request session projection".to_owned(),
            )
        })?;
        if !stored.flash.is_empty() || stored.errors.is_some() {
            session
                .insert(KEY, stored)
                .await
                .map_err(|error| TowerSessionTransientError(error.to_string()))?;
        }
        Ok(())
    }
}
