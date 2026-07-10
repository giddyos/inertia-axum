//! Request parsing and partial-reload selection.

mod context;
mod header_list;
mod selection;

pub use context::RequestContext;
pub(crate) use selection::{EffectiveRequest, SelectionMode, SelectionPlan};
