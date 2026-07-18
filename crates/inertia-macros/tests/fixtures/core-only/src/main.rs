use inertia_core::InertiaPage as _;

#[derive(inertia_core::InertiaPage)]
#[inertia(component = "Home")]
struct Home {
    message: String,
}

fn main() {
    let page = Home {
        message: "framework neutral".to_owned(),
    };
    let _: inertia_core::PendingPage = page.into_pending_page();
}
