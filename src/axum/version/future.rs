//! Allocation-free enum future used by version middleware.

use axum::response::Response;
use pin_project_lite::pin_project;
use std::task::{Context, Poll};

pin_project! {
    #[project = VersionFutureProj]
    pub enum VersionFuture<F, E> {
        Inner { #[pin] future: F },
        Ready { result: Option<Result<Response, E>> },
    }
}

impl<F, E> std::future::Future for VersionFuture<F, E>
where
    F: std::future::Future<Output = Result<Response, E>>,
{
    type Output = Result<Response, E>;

    fn poll(self: std::pin::Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Self::Output> {
        match self.project() {
            VersionFutureProj::Inner { future } => future.poll(context),
            VersionFutureProj::Ready { result } => Poll::Ready(
                result
                    .take()
                    .expect("ready version future polled after completion"),
            ),
        }
    }
}
