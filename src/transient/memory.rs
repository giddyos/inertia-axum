use super::{StoredTransient, TransientData, TransientRequest, TransientStore};
use axum::response::Response;
use std::{
    collections::HashMap,
    convert::Infallible,
    sync::{Arc, Mutex},
};

/// Deterministic in-memory transient store intended for tests.
#[derive(Clone, Default)]
pub struct MemoryTransient(Arc<Mutex<HashMap<Box<str>, StoredTransient>>>);

impl MemoryTransient {
    /// Creates an empty test store.
    pub fn new() -> Self {
        Self::default()
    }
}

impl TransientStore for MemoryTransient {
    type Error = Infallible;
    async fn load(&self, request: TransientRequest<'_>) -> Result<TransientData, Self::Error> {
        let scope: Box<str> = request.test_scope().unwrap_or("default").into();
        let stored = self
            .0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .remove(&scope)
            .unwrap_or_default();
        Ok(TransientData::loaded(stored, scope))
    }
    async fn commit(
        &self,
        _response: &mut Response,
        data: TransientData,
    ) -> Result<(), Self::Error> {
        let scope: Box<str> = data.scope().into();
        let stored = data.into_stored();
        let mut values = self
            .0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if stored.flash.is_empty() && stored.errors.is_none() {
            values.remove(&scope);
        } else {
            values.insert(scope, stored);
        }
        Ok(())
    }
}
