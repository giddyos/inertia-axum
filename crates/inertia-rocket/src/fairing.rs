//! Rocket fairing for state installation, assets, and pending response finalization.

use crate::{
    assets::{self, AssetState},
    boundary,
    response::{EarlyResponseSlot, PendingSlot, core_response},
};
use rocket::{
    Build, Request, Response, Rocket,
    fairing::{self, Fairing, Info, Kind},
};

/// Installs a framework-neutral Inertia application in Rocket.
pub struct InertiaFairing {
    app: inertia_core::InertiaApp,
}

impl InertiaFairing {
    /// Creates an installation fairing for `app`.
    pub fn new(app: inertia_core::InertiaApp) -> Self {
        Self { app }
    }
}

#[rocket::async_trait]
impl Fairing for InertiaFairing {
    fn info(&self) -> Info {
        Info {
            name: "Inertia",
            kind: Kind::Ignite | Kind::Response,
        }
    }

    async fn on_ignite(&self, rocket: Rocket<Build>) -> fairing::Result {
        if rocket.state::<inertia_core::InertiaApp>().is_some() {
            rocket::error!(
                "inertia-rocket is installed more than once; attach exactly one InertiaFairing"
            );
            return Err(rocket);
        }
        let public_path = self.app.asset_public_path().to_owned();
        if !public_path.starts_with('/') || public_path.contains(['?', '#']) {
            rocket::error!("invalid Inertia asset public path for Rocket: {public_path}");
            return Err(rocket);
        }
        let source = self.app.asset_source().cloned();
        let rocket = rocket.manage(self.app.clone());
        let rocket = if let Some(source) = source {
            rocket
                .manage(AssetState(source))
                .mount(public_path, assets::routes())
        } else {
            rocket
        };
        Ok(rocket)
    }
    async fn on_response<'r>(&self, request: &'r Request<'_>, response: &mut Response<'r>) {
        if let Some(early) = request.local_cache(EarlyResponseSlot::default).take() {
            replace_response(response, early);
            return;
        }
        let Some(pending) = request.local_cache(PendingSlot::default).take() else {
            return;
        };
        let parts = match boundary::request_parts(request) {
            Ok(parts) => parts,
            Err(error) => {
                let error = inertia_core::CoreResponse::bytes(
                    http::StatusCode::BAD_REQUEST,
                    error.into_bytes(),
                );
                replace_response(response, error);
                return;
            }
        };
        let prepared = match self.app.prepare_request(parts, None).await {
            Ok(inertia_core::VersionCheck::Proceed(prepared)) => *prepared,
            Ok(inertia_core::VersionCheck::Mismatch(mismatch)) => {
                replace_response(response, mismatch);
                return;
            }
            Err(error) => {
                replace_response(response, error.into_response());
                return;
            }
        };
        #[cfg(feature = "ssr")]
        let finalized = prepared.finalize_with_ssr(pending, None).await;
        #[cfg(not(feature = "ssr"))]
        let finalized = prepared.finalize(pending).await;
        replace_response(response, finalized);
    }
}

fn replace_response(target: &mut Response<'_>, core: inertia_core::CoreResponse) {
    if let Ok(response) = core_response(core) {
        *target = response;
    } else {
        target.set_status(rocket::http::Status::InternalServerError);
        target.set_sized_body(
            "invalid framework-neutral response header".len(),
            std::io::Cursor::new("invalid framework-neutral response header"),
        );
    }
}
