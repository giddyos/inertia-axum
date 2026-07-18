use super::{StoredTransient, TransientData, TransientRequest, TransientStore};
use crate::CoreResponse;
use cookie::{Cookie, CookieJar, Key, SameSite};
use http::{HeaderValue, header::SET_COOKIE};
use sha2::{Digest, Sha512};
use std::{error::Error, fmt};

const NAME: &str = "__Host-inertia_transient";

/// Encrypted, authenticated cookie transient storage.
#[derive(Clone)]
pub struct CookieTransient {
    key: Key,
}
impl CookieTransient {
    /// Creates the secure production adapter. Supply at least 32 random bytes.
    pub fn encrypted(key: impl AsRef<[u8]>) -> Self {
        let material = Sha512::digest(key.as_ref());
        Self {
            key: Key::from(material.as_slice()),
        }
    }
}

#[derive(Debug)]
pub struct CookieTransientError(String);
impl fmt::Display for CookieTransientError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
impl Error for CookieTransientError {}

impl TransientStore for CookieTransient {
    type Error = CookieTransientError;
    async fn load(&self, request: TransientRequest<'_>) -> Result<TransientData, Self::Error> {
        let mut jar = CookieJar::new();
        if let Some(header) = request
            .cookie_header()
            .and_then(|value| value.to_str().ok())
        {
            for cookie in header.split(';').map(str::trim) {
                if let Ok(cookie) = Cookie::parse(cookie.to_owned()) {
                    jar.add_original(cookie);
                }
            }
        }
        let stored = jar.private(&self.key).get(NAME).map_or_else(
            || Ok(StoredTransient::default()),
            |cookie| {
                serde_json::from_str(cookie.value()).map_err(|error| {
                    CookieTransientError(format!("invalid encrypted transient payload: {error}"))
                })
            },
        )?;
        Ok(TransientData::loaded(stored, "cookie"))
    }
    async fn commit(
        &self,
        response: &mut CoreResponse,
        data: TransientData,
    ) -> Result<(), Self::Error> {
        let value = serde_json::to_string(&data.into_stored()).map_err(|error| {
            CookieTransientError(format!("failed to serialize transient payload: {error}"))
        })?;
        let mut jar = CookieJar::new();
        jar.private_mut(&self.key).add(
            Cookie::build((NAME, value))
                .path("/")
                .http_only(true)
                .secure(true)
                .same_site(SameSite::Lax)
                .build(),
        );
        let cookie = jar.delta().next().ok_or_else(|| {
            CookieTransientError("encrypted cookie jar produced no commit".to_owned())
        })?;
        response.headers_mut().append(
            SET_COOKIE,
            HeaderValue::from_str(&cookie.to_string()).map_err(|error| {
                CookieTransientError(format!("invalid transient Set-Cookie header: {error}"))
            })?,
        );
        Ok(())
    }
}
