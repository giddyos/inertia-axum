use axum::{
    routing::{get, post},
    Router,
};
use inertia_axum::prelude::*;
use inertia_axum::{Errors, Location, ScrollPage};
use serde::{Deserialize, Serialize};
use std::{
    convert::Infallible,
    io,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Item {
    pub id: u64,
    pub label: String,
}

#[derive(InertiaPage)]
#[inertia(
    component = "Incidents/Show",
    rename_all = "camelCase",
    encrypt_history
)]
pub struct IncidentShowPage {
    pub incident: Item,
    pub timeline: Prop<ScrollPage<Item>>,
    pub telemetry: Prop<Item>,
    pub affected_machines: Prop<Vec<Item>>,
    pub raw_controller_payloads: Prop<Vec<Item>>,
    pub playbooks: Prop<Vec<Item>>,
    pub participants: Prop<Vec<Item>>,
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

#[derive(Deserialize, InertiaForm)]
#[inertia(
    validate_with = "validate_incident",
    error_bag = "createIncident",
    old_input
)]
pub struct CreateIncident {
    pub title: String,
}

#[derive(Clone)]
pub struct FixtureState {
    pub raw_calls: Arc<AtomicUsize>,
    pub fail_telemetry: bool,
}

pub fn app(state: FixtureState) -> Router {
    let page_state = state.clone();
    async fn create(Validated(input): Validated<CreateIncident>) -> Redirect {
        Redirect::to("/incidents/1").flash("toast", format!("Created {}", input.title))
    }
    Router::new()
        .route(
            "/incidents/1",
            get(move || {
                let state = page_state.clone();
                async move {
                    let raw = state.raw_calls.clone();
                    let fails = state.fail_telemetry;
                    IncidentShowPage {
                        incident: item(1, "Compressor trip"),
                        timeline: scroll(
                            ScrollPage::new(vec![item(1, "Detected")], 1)
                                .next(2)
                                .page_name("timeline"),
                        )
                        .match_on("id"),
                        telemetry: defer(move || async move {
                            if fails {
                                Err(io::Error::other("offline"))
                            } else {
                                Ok(item(2, "Nominal"))
                            }
                        })
                        .group("telemetry")
                        .rescue(),
                        affected_machines: defer(|| async {
                            Ok::<_, Infallible>(vec![item(3, "Press 7")])
                        })
                        .group("telemetry"),
                        raw_controller_payloads: optional(move || async move {
                            raw.fetch_add(1, Ordering::SeqCst);
                            Ok::<_, Infallible>(vec![item(4, "raw")])
                        }),
                        playbooks: once(|| async {
                            Ok::<_, Infallible>(vec![item(5, "Evacuate")])
                        })
                        .key("incident-playbooks:v3"),
                        participants: merge(vec![item(6, "Ada")]).append().match_on("id"),
                    }
                }
            }),
        )
        .route("/incidents", post(create))
        .route(
            "/machines/1/maintenance",
            get(|| async { Location::external("https://maintenance.example/machines/1") }),
        )
        .inertia(
            InertiaApp::default_root()
                .version(42_u64)
                .transient(MemoryTransient::new())
                .build()
                .unwrap(),
        )
}

fn item(id: u64, label: &str) -> Item {
    Item {
        id,
        label: label.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use inertia_axum_test::TestApp;
    use serde_json::json;

    #[tokio::test]
    async fn command_board_exercises_protocol_policies() {
        let raw = Arc::new(AtomicUsize::new(0));
        let app = TestApp::new(app(FixtureState {
            raw_calls: raw.clone(),
            fail_telemetry: false,
        }))
        .with_version(42_u64);
        let initial = app
            .inertia_get("/incidents/1")
            .send()
            .await
            .assert_page::<IncidentShowPage>();
        initial
            .assert_scroll(IncidentShowPage::TIMELINE)
            .assert_appends(IncidentShowPage::PARTICIPANTS)
            .assert_matches_on(IncidentShowPage::PARTICIPANTS, "id")
            .assert_encrypts_history()
            .assert_version(42_u64);
        assert_eq!(raw.load(Ordering::SeqCst), 0);
        app.inertia_get("/incidents/1")
            .only(IncidentShowPage::RAW_CONTROLLER_PAYLOADS)
            .send()
            .await
            .assert_page::<IncidentShowPage>();
        assert_eq!(raw.load(Ordering::SeqCst), 1);
        app.inertia_get("/incidents/1")
            .version("stale")
            .send()
            .await
            .assert_location_conflict("/incidents/1");
        app.inertia_get("/machines/1/maintenance")
            .send()
            .await
            .assert_location_conflict("https://maintenance.example/machines/1");
        let invalid = app
            .inertia_post("/incidents")
            .header("referer", "/incidents/1")
            .json(&json!({"title":""}))
            .error_bag("createIncident")
            .send()
            .await
            .assert_see_other("/incidents/1");
        invalid
            .follow()
            .await
            .assert_page::<IncidentShowPage>()
            .assert_error("createIncident.title");
        let created = app
            .inertia_post("/incidents")
            .json(&json!({"title":"Line failure"}))
            .send()
            .await
            .assert_see_other("/incidents/1");
        let page = created.follow().await.assert_page::<IncidentShowPage>();
        let toast: String = page.flash("toast");
        assert!(toast.contains("Line failure"));
        app.inertia_get("/incidents/1")
            .send()
            .await
            .assert_page::<IncidentShowPage>()
            .assert_no_flash("toast");
        assert_eq!(app.history(), vec!["/incidents/1", "/incidents/1"]);
    }

    #[tokio::test]
    async fn unavailable_telemetry_is_rescued() {
        let app = TestApp::new(app(FixtureState {
            raw_calls: Arc::new(AtomicUsize::new(0)),
            fail_telemetry: true,
        }))
        .with_version(42_u64);
        let page = app
            .inertia_get("/incidents/1")
            .only(IncidentShowPage::TELEMETRY)
            .send()
            .await
            .assert_page::<IncidentShowPage>();
        page.assert_missing(IncidentShowPage::TELEMETRY)
            .assert_rescued(IncidentShowPage::TELEMETRY);
    }
}
