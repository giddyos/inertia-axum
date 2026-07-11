use axum::{
    extract::{Path, State},
    routing::{get, post},
    Router,
};
use inertia_axum::{prelude::*, Errors, Location, ScrollPage};
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
pub struct Datum {
    pub id: u64,
    pub name: String,
}

#[derive(InertiaPage)]
#[inertia(
    component = "Anomalies/Show",
    rename_all = "camelCase",
    encrypt_history
)]
pub struct AnomalyShowPage {
    pub anomaly: Datum,
    pub timeline: Prop<ScrollPage<Datum>>,
    pub telemetry: Prop<Datum>,
    pub affected_instruments: Prop<Vec<Datum>>,
    pub raw_frames: Prop<Vec<Datum>>,
    pub calibration_profiles: Prop<Vec<Datum>>,
    pub collaborators: Prop<Vec<Datum>>,
}

#[derive(Clone)]
pub struct AppState {
    pub raw_calls: Arc<AtomicUsize>,
    pub fail_telemetry: bool,
}

async fn show(State(state): State<AppState>, Path(id): Path<u64>) -> AnomalyShowPage {
    let raw_calls = state.raw_calls;
    let fail_telemetry = state.fail_telemetry;

    AnomalyShowPage {
        anomaly: datum(id, "Transit drift"),
        timeline: scroll(
            ScrollPage::new(vec![datum(2, "Observed")], 2)
                .previous(1)
                .page_name("timeline"),
        )
        .match_on("id"),
        telemetry: defer(move || async move {
            if fail_telemetry {
                Err(io::Error::other("unavailable"))
            } else {
                Ok(datum(3, "Spectrum"))
            }
        })
        .group("science")
        .rescue(),
        affected_instruments: defer(|| async { Ok::<_, Infallible>(vec![datum(4, "Array A")]) })
            .group("science"),
        raw_frames: optional(move || async move {
            raw_calls.fetch_add(1, Ordering::SeqCst);
            Ok::<_, Infallible>(vec![datum(5, "Frame")])
        }),
        calibration_profiles: once(|| async { Ok::<_, Infallible>(vec![datum(6, "Baseline")]) })
            .key("calibration-profiles:v3"),
        collaborators: merge(vec![datum(7, "Grace")]).deep().match_on("id"),
    }
}

#[derive(Deserialize, InertiaForm)]
#[inertia(
    validate_with = "validate_anomaly",
    error_bag = "createAnomaly",
    old_input,
    redact = "sensitive_token"
)]
pub struct CreateAnomaly {
    pub title: String,
    pub sensitive_token: Option<String>,
}

fn validate_anomaly(input: &CreateAnomaly) -> Result<(), Errors> {
    if input.title.is_empty() {
        Err(Errors::field("title", "required"))
    } else {
        Ok(())
    }
}

async fn store(Validated(input): Validated<CreateAnomaly>) -> Redirect {
    let _ = input.sensitive_token;
    Redirect::to("/anomalies/1").flash("toast", "Anomaly created")
}

async fn console(Path(id): Path<u64>) -> Location {
    Location::external(format!("https://console.example/telescopes/{id}"))
}

pub fn app(state: AppState) -> Router {
    let inertia = InertiaApp::default_root()
        .transient(MemoryTransient::new())
        .build()
        .expect("fixture Inertia configuration should be valid");

    Router::new()
        .route("/anomalies/{id}", get(show))
        .route("/anomalies", post(store))
        .route("/telescopes/{id}/console", get(console))
        .with_state(state)
        .inertia(inertia)
}

fn datum(id: u64, name: &str) -> Datum {
    Datum {
        id,
        name: name.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use inertia_axum_test::TestApp;
    use serde_json::json;

    fn test_app(fail_telemetry: bool) -> (TestApp, Arc<AtomicUsize>) {
        let raw_calls = Arc::new(AtomicUsize::new(0));
        let app = TestApp::new(app(AppState {
            raw_calls: raw_calls.clone(),
            fail_telemetry,
        }));
        (app, raw_calls)
    }

    #[tokio::test]
    async fn protocol_metadata_and_selection_are_preserved() {
        let (app, raw_calls) = test_app(false);
        let initial = app
            .inertia_get("/anomalies/1")
            .send()
            .await
            .assert_page::<AnomalyShowPage>();

        initial
            .assert_deferred("science", AnomalyShowPage::TELEMETRY)
            .assert_deferred("science", AnomalyShowPage::AFFECTED_INSTRUMENTS)
            .assert_once(
                "calibration-profiles:v3",
                AnomalyShowPage::CALIBRATION_PROFILES,
            )
            .assert_appends(AnomalyShowPage::TIMELINE)
            .assert_deep_merges(AnomalyShowPage::COLLABORATORS)
            .assert_matches_on(AnomalyShowPage::COLLABORATORS, "id")
            .assert_missing(AnomalyShowPage::RAW_FRAMES);
        assert_eq!(raw_calls.load(Ordering::SeqCst), 0);

        app.inertia_get("/anomalies/1")
            .only(AnomalyShowPage::RAW_FRAMES)
            .send()
            .await
            .assert_page::<AnomalyShowPage>();
        assert_eq!(raw_calls.load(Ordering::SeqCst), 1);

        app.inertia_get("/anomalies/1")
            .only(AnomalyShowPage::TIMELINE)
            .scroll_intent("prepend")
            .send()
            .await
            .assert_page::<AnomalyShowPage>()
            .assert_prepends(AnomalyShowPage::TIMELINE);
        app.inertia_get("/anomalies/1")
            .only(AnomalyShowPage::TIMELINE)
            .reset(AnomalyShowPage::TIMELINE)
            .send()
            .await
            .assert_page::<AnomalyShowPage>()
            .assert_reset(AnomalyShowPage::TIMELINE);
    }

    #[tokio::test]
    async fn rescue_and_redacted_old_input_are_preserved() {
        let (app, _) = test_app(true);
        app.inertia_get("/anomalies/1")
            .only(AnomalyShowPage::TELEMETRY)
            .send()
            .await
            .assert_page::<AnomalyShowPage>()
            .assert_missing(AnomalyShowPage::TELEMETRY)
            .assert_rescued(AnomalyShowPage::TELEMETRY);

        let response = app
            .inertia_post("/anomalies")
            .header("referer", "/anomalies/1")
            .json(&json!({"title":"", "sensitive_token":"secret"}))
            .send()
            .await
            .assert_see_other("/anomalies/1");
        let page = response.follow().await.assert_page::<AnomalyShowPage>();

        page.assert_error("createAnomaly.title");
        assert_eq!(page.value()["props"]["oldInput"]["title"], "");
        assert!(!page.value().to_string().contains("secret"));
    }
}
