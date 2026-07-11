use inertia_axum_macros::InertiaProps;

#[derive(InertiaProps)]
#[inertia(rename_all = "camelCase")]
struct Shared {
    app_name: String,
    #[inertia(skip)]
    secret: String,
}

fn main() {
    let _ = ia::IntoInertiaProps::into_inertia_props(Shared {
        app_name: "Demo".to_owned(),
        secret: "hidden".to_owned(),
    });
}
