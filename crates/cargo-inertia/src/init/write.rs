//! Adjacent staged writing and atomic commit.

use std::fs;

use crate::{
    error::CliError,
    init::{plan::ScaffoldPlan, validate::validate_relative_path},
};

/// Writes a complete plan to adjacent staging, then renames it into place.
pub fn commit(plan: &ScaffoldPlan) -> Result<(), CliError> {
    if plan.destination.exists() {
        return Err(CliError::FrontendExists(plan.destination.clone()));
    }
    let parent = plan
        .destination
        .parent()
        .ok_or_else(|| CliError::InvalidDestination(plan.destination.clone()))?;
    fs::create_dir_all(parent)?;
    let staging = tempfile::Builder::new()
        .prefix(".cargo-inertia-")
        .tempdir_in(parent)?;
    for file in &plan.files {
        validate_relative_path(&file.relative_path)?;
        let output = staging.path().join(&file.relative_path);
        if let Some(parent) = output.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(output, &file.contents)?;
    }
    let staged_path = staging.keep();
    if let Err(source) = fs::rename(&staged_path, &plan.destination) {
        let _ = fs::remove_dir_all(&staged_path);
        return Err(CliError::CommitScaffold {
            from: staged_path,
            to: plan.destination.clone(),
            source,
        });
    }
    Ok(())
}
