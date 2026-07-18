#[derive(inertia_axum::InertiaPage)]
#[inertia(component = "Home")]
struct Home {
    message: String,
}

fn main() {
    let page = Home {
        message: "Axum adapter".to_owned(),
    };
    let _: inertia_axum::PendingPage = inertia_axum::PendingPage::typed(page);
}
