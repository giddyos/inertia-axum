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

fn validate(input: &CreateAnomaly) -> Result<(), Errors> {
    if input.title.is_empty() {
        Err(Errors::field("title", "required"))
    } else {
        Ok(())
    }
}

#[derive(Deserialize, InertiaForm)]
#[inertia(
    validate_with = "validate",
    error_bag = "createAnomaly",
    old_input,
    redact = "sensitive_token"
)]
pub struct CreateAnomaly {
    pub title: String,
    pub sensitive_token: Option<String>,
}

#[derive(Clone)]
pub struct State {
    pub raw_calls: Arc<AtomicUsize>,
    pub fail: bool,
}

pub fn app(state: State) -> Router {
    let show_state = state;
    async fn create(Validated(input): Validated<CreateAnomaly>) -> Redirect {
        let _ = input.sensitive_token;
        Redirect::to("/anomalies/1").flash("toast", "Anomaly created")
    }
    Router::new()
        .route(
            "/anomalies/1",
            get(move || {
                let state = show_state.clone();
                async move {
                    let raw = state.raw_calls;
                    let fail = state.fail;
                    AnomalyShowPage {
                        anomaly: datum(1, "Transit drift"),
                        timeline: scroll(
                            ScrollPage::new(vec![datum(2, "Observed")], 2)
                                .previous(1)
                                .page_name("timeline"),
                        )
                        .match_on("id"),
                        telemetry: defer(move || async move {
                            if fail {
                                Err(io::Error::other("unavailable"))
                            } else {
                                Ok(datum(3, "Spectrum"))
                            }
                        })
                        .group("science")
                        .rescue(),
                        affected_instruments: defer(|| async {
                            Ok::<_, Infallible>(vec![datum(4, "Array A")])
                        })
                        .group("science"),
                        raw_frames: optional(move || async move {
                            raw.fetch_add(1, Ordering::SeqCst);
                            Ok::<_, Infallible>(vec![datum(5, "Frame")])
                        }),
                        calibration_profiles: once(|| async {
                            Ok::<_, Infallible>(vec![datum(6, "Baseline")])
                        })
                        .key("calibration-profiles:v3"),
                        collaborators: merge(vec![datum(7, "Grace")]).deep().match_on("id"),
                    }
                }
            }),
        )
        .route("/anomalies", post(create))
        .route(
            "/telescopes/1/console",
            get(|| async { Location::external("https://console.example/telescopes/1") }),
        )
        .inertia(
            InertiaApp::default_root()
                .transient(MemoryTransient::new())
                .build()
                .unwrap(),
        )
}

fn datum(id: u64, name: &str) -> Datum {
    Datum {
        id,
        name: name.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use inertia_axum_test::TestApp;
    use serde_json::json;

    #[tokio::test]
    async fn alternate_domain_covers_loading_merge_scroll_once_and_location() {
        let raw = Arc::new(AtomicUsize::new(0));
        let app = TestApp::new(app(State {
            raw_calls: raw.clone(),
            fail: false,
        }));
        let initial = app
            .inertia_get("/anomalies/1")
            .send()
            .await
            .assert_page::<AnomalyShowPage>();
        initial
            .assert_missing(AnomalyShowPage::TELEMETRY)
            .assert_missing(AnomalyShowPage::AFFECTED_INSTRUMENTS)
            .assert_missing(AnomalyShowPage::RAW_FRAMES)
            .assert_deep_merges(AnomalyShowPage::COLLABORATORS)
            .assert_matches_on(AnomalyShowPage::COLLABORATORS, "id");
        assert_eq!(raw.load(Ordering::SeqCst), 0);
        let science = app
            .inertia_get("/anomalies/1")
            .only(AnomalyShowPage::TELEMETRY)
            .only(AnomalyShowPage::AFFECTED_INSTRUMENTS)
            .send()
            .await
            .assert_page::<AnomalyShowPage>();
        let _: Datum = science.prop(AnomalyShowPage::TELEMETRY);
        let _: Vec<Datum> = science.prop(AnomalyShowPage::AFFECTED_INSTRUMENTS);
        app.inertia_get("/anomalies/1")
            .only(AnomalyShowPage::RAW_FRAMES)
            .send()
            .await
            .assert_page::<AnomalyShowPage>();
        assert_eq!(raw.load(Ordering::SeqCst), 1);
        app.inertia_get("/anomalies/1")
            .except_once("calibration-profiles:v3")
            .send()
            .await
            .assert_page::<AnomalyShowPage>()
            .assert_missing(AnomalyShowPage::CALIBRATION_PROFILES);
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
        app.inertia_get("/telescopes/1/console")
            .send()
            .await
            .assert_location_conflict("https://console.example/telescopes/1");
    }

    #[tokio::test]
    async fn rescue_and_redacted_bagged_validation_are_visible() {
        let app = TestApp::new(app(State {
            raw_calls: Arc::new(AtomicUsize::new(0)),
            fail: true,
        }));
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
        assert!(!page.value().to_string().contains("secret"));
    }
}
