use axum::{
    extract::{Path, State},
    routing::{get, post},
    Router,
};
use inertia_axum::{prelude::*, Errors, Location, ScrollPage};
use serde::{Deserialize, Serialize};
use std::io;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Item {
    pub id: u64,
    pub label: String,
}

#[derive(InertiaPage)]
#[inertia(component = "Incidents/Show", rename_all = "camelCase")]
pub struct IncidentShowPage {
    pub incident: Item,
    pub timeline: Prop<ScrollPage<Item>>,
    pub telemetry: Prop<Item>,
    pub participants: Prop<Vec<Item>>,
}

#[derive(Clone)]
pub struct AppState {
    pub fail_telemetry: bool,
}

async fn show(State(state): State<AppState>, Path(id): Path<u64>) -> IncidentShowPage {
    IncidentShowPage {
        incident: item(id, "Compressor trip"),
        timeline: scroll(
            ScrollPage::new(vec![item(1, "Detected")], 1)
                .next(2)
                .page_name("timeline"),
        )
        .match_on("id"),

        // Failed deferred props are recorded as rescued instead of failing the page.
        telemetry: defer(move || async move {
            if state.fail_telemetry {
                Err(io::Error::other("offline"))
            } else {
                Ok(item(2, "Nominal"))
            }
        })
        .group("telemetry")
        .rescue(),
        participants: merge(vec![item(6, "Ada")]).append().match_on("id"),
    }
}

#[derive(Deserialize, InertiaForm)]
#[inertia(validate_with = "validate_incident", error_bag = "createIncident")]
pub struct CreateIncident {
    pub title: String,
}

fn validate_incident(input: &CreateIncident) -> Result<(), Errors> {
    if input.title.trim().len() < 3 {
        Err(Errors::field(
            "title",
            "title must contain at least 3 characters",
        ))
    } else {
        Ok(())
    }
}

async fn store(Validated(input): Validated<CreateIncident>) -> Redirect {
    Redirect::to("/incidents/1").flash("toast", format!("Created {}", input.title))
}

async fn maintenance(Path(id): Path<u64>) -> Location {
    Location::external(format!("https://maintenance.example/machines/{id}"))
}

pub fn app(state: AppState) -> Router {
    let inertia = InertiaApp::default_root()
        .transient(MemoryTransient::new())
        .build()
        .expect("fixture Inertia configuration should be valid");

    Router::new()
        .route("/incidents/{id}", get(show))
        .route("/incidents", post(store))
        .route("/machines/{id}/maintenance", get(maintenance))
        .with_state(state)
        .inertia(inertia)
}

fn item(id: u64, label: &str) -> Item {
    Item {
        id,
        label: label.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use inertia_axum_test::TestApp;

    fn test_app(fail_telemetry: bool) -> TestApp {
        TestApp::new(app(AppState { fail_telemetry }))
    }

    #[tokio::test]
    async fn deferred_telemetry_is_advertised_and_loaded() {
        let app = test_app(false);
        let initial = app
            .inertia_get("/incidents/1")
            .send()
            .await
            .assert_page::<IncidentShowPage>();

        initial
            .assert_deferred("telemetry", IncidentShowPage::TELEMETRY)
            .assert_missing(IncidentShowPage::TELEMETRY);

        let partial = app
            .inertia_get("/incidents/1")
            .only(IncidentShowPage::TELEMETRY)
            .send()
            .await
            .assert_page::<IncidentShowPage>();
        let telemetry: Item = partial.prop(IncidentShowPage::TELEMETRY);

        assert_eq!(telemetry.label, "Nominal");
        partial.assert_missing(IncidentShowPage::INCIDENT);
    }

    #[tokio::test]
    async fn failed_telemetry_is_rescued() {
        test_app(true)
            .inertia_get("/incidents/1")
            .only(IncidentShowPage::TELEMETRY)
            .send()
            .await
            .assert_page::<IncidentShowPage>()
            .assert_missing(IncidentShowPage::TELEMETRY)
            .assert_rescued(IncidentShowPage::TELEMETRY);
    }

    #[tokio::test]
    async fn timeline_emits_scroll_metadata() {
        test_app(false)
            .inertia_get("/incidents/1")
            .send()
            .await
            .assert_page::<IncidentShowPage>()
            .assert_scroll(IncidentShowPage::TIMELINE);
    }

    #[tokio::test]
    async fn external_maintenance_links_use_location_responses() {
        test_app(false)
            .inertia_get("/machines/7/maintenance")
            .send()
            .await
            .assert_location_conflict("https://maintenance.example/machines/7");
    }
}
