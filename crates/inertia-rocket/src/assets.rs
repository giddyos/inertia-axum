//! Rocket conversion and routes for framework-neutral asset responses.

use inertia_core::{AssetBody, AssetRequest, AssetResponse, AssetSource};
use rocket::{
    Request, State,
    http::Status,
    request::{FromRequest, Outcome},
    response::{Responder, Response},
};
use std::{io::Cursor, path::PathBuf, sync::Arc};

pub(crate) struct AssetState(pub(crate) Arc<dyn AssetSource>);

pub(crate) struct RocketAssetResponse(AssetResponse);

pub(crate) struct RawRequest(inertia_core::RequestParts);

#[rocket::async_trait]
impl<'r> FromRequest<'r> for RawRequest {
    type Error = String;

    async fn from_request(request: &'r Request<'_>) -> Outcome<Self, Self::Error> {
        match crate::boundary::request_parts(request) {
            Ok(parts) => Outcome::Success(Self(parts)),
            Err(error) => Outcome::Error((Status::BadRequest, error)),
        }
    }
}

impl<'r, 'o: 'r> Responder<'r, 'o> for RocketAssetResponse {
    fn respond_to(self, _request: &'r Request<'_>) -> rocket::response::Result<'o> {
        let mut response = Response::build();
        response.status(Status::new(self.0.status.as_u16()));
        for (name, value) in &self.0.headers {
            let value = value.to_str().map_err(|_| Status::InternalServerError)?;
            response.raw_header_adjoin(name.as_str().to_owned(), value.to_owned());
        }
        match self.0.body {
            AssetBody::Empty => {}
            AssetBody::Bytes(bytes) => {
                response.sized_body(bytes.len(), Cursor::new(bytes));
            }
            AssetBody::Static(bytes) => {
                response.sized_body(bytes.len(), Cursor::new(bytes));
            }
        }
        response.ok()
    }
}

fn asset(
    path: PathBuf,
    request: &inertia_core::RequestParts,
    source: &State<AssetState>,
) -> Option<RocketAssetResponse> {
    let path = path.to_str()?;
    source
        .0
        .get(AssetRequest {
            method: request.method(),
            path,
            headers: request.headers(),
        })
        .map(RocketAssetResponse)
}

#[rocket::get("/<path..>", rank = 100)]
pub(crate) fn get_asset(
    path: PathBuf,
    request: RawRequest,
    source: &State<AssetState>,
) -> Option<RocketAssetResponse> {
    asset(path, &request.0, source)
}

#[rocket::head("/<path..>", rank = 100)]
pub(crate) fn head_asset(
    path: PathBuf,
    request: RawRequest,
    source: &State<AssetState>,
) -> Option<RocketAssetResponse> {
    asset(path, &request.0, source)
}

macro_rules! asset_method_route {
    ($name:ident, $attribute:ident) => {
        #[rocket::$attribute("/<path..>", rank = 100)]
        pub(crate) fn $name(
            path: PathBuf,
            request: RawRequest,
            source: &State<AssetState>,
        ) -> Option<RocketAssetResponse> {
            asset(path, &request.0, source)
        }
    };
}

asset_method_route!(post_asset, post);
asset_method_route!(put_asset, put);
asset_method_route!(patch_asset, patch);
asset_method_route!(delete_asset, delete);
asset_method_route!(options_asset, options);

pub(crate) fn routes() -> Vec<rocket::Route> {
    rocket::routes![
        get_asset,
        head_asset,
        post_asset,
        put_asset,
        patch_asset,
        delete_asset,
        options_asset
    ]
}
