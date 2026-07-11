use ia::{defer, Prop};
use inertia_axum_macros::InertiaPage;
use std::convert::Infallible;

#[derive(InertiaPage)]
#[inertia(component = "Users/Index", rename_all = "camelCase", encrypt_history)]
struct Users<T: serde::Serialize + Send + 'static> {
    users: Vec<T>,
    count: Prop<u64>,
    #[inertia(rename = "canCreate")]
    can_create: bool,
    #[inertia(skip)]
    marker: std::marker::PhantomData<T>,
}

fn main() {
    let page = Users::<String> {
        users: vec![],
        count: defer(|| async { Ok::<_, Infallible>(1) }),
        can_create: true,
        marker: std::marker::PhantomData,
    };
    let _ = ia::InertiaPage::into_pending_page(page);
    let _: ia::PropKey<u64> = Users::<String>::COUNT;
}
