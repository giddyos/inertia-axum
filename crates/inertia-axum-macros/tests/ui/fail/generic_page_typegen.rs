use inertia_axum_macros::InertiaPage;

#[derive(InertiaPage)]
#[inertia(component = "Items/Index")]
struct Items<T: serde::Serialize + Send + 'static> {
    item: T,
}

fn main() {}
