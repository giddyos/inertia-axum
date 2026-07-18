/// The active SSR backend implementation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SsrBackendKind {
    /// The Vite development server.
    Vite,
    /// A managed Node child process.
    ManagedNode,
    /// An externally supervised Node server.
    External,
}

/// A stable classification for an SSR runtime failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(missing_docs)]
pub enum SsrFailureKind {
    Unavailable,
    Overloaded,
    Timeout,
    Transport,
    InvalidResponse,
    Render,
    ResponseTooLarge,
    ProcessExited,
}

/// The latest locally recorded SSR backend state.
#[derive(Clone, Debug, PartialEq, Eq)]
#[allow(missing_docs)]
pub enum SsrHealth {
    Disabled,
    Starting {
        backend: SsrBackendKind,
    },
    Ready {
        backend: SsrBackendKind,
    },
    Degraded {
        backend: SsrBackendKind,
        last_failure: SsrFailureKind,
    },
    Unavailable {
        backend: SsrBackendKind,
    },
}
