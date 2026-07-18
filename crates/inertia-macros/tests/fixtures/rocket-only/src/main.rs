#[derive(inertia_rocket::InertiaPage)]
#[inertia(component = "Home")]
struct Home {
    message: String,
}

fn main() {
    let page = Home {
        message: "Rocket adapter".to_owned(),
    };
    let _: inertia_rocket::PendingPage = inertia_rocket::PendingPage::typed(page);
}
