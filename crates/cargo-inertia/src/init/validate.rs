//! Validation of fully rendered scaffold plans.

use std::{
    collections::BTreeSet,
    path::{Component, Path},
};

use crate::{error::CliError, framework::Framework, init::plan::ScaffoldPlan};

/// Validates safe paths and generated source invariants before writing anything.
pub fn validate(plan: &ScaffoldPlan, framework: Framework) -> Result<(), CliError> {
    let mut paths = BTreeSet::new();
    for file in &plan.files {
        validate_relative_path(&file.relative_path)?;
        if !paths.insert(&file.relative_path) {
            return Err(CliError::UnsafeOutputPath(file.relative_path.clone()));
        }
        let text = std::str::from_utf8(&file.contents)
            .map_err(|_| CliError::UnsafeOutputPath(file.relative_path.clone()))?;
        if ["[[=", "[[%", "[[#"]
            .iter()
            .any(|marker| text.contains(marker))
        {
            return Err(CliError::Message(format!(
                "unrendered template marker in {}",
                file.relative_path.display()
            )));
        }
    }
    let package = plan
        .files
        .iter()
        .find(|file| file.relative_path == Path::new("package.json"))
        .ok_or_else(|| CliError::Message("missing package.json".to_owned()))?;
    let package_text = std::str::from_utf8(&package.contents)
        .map_err(|_| CliError::UnsafeOutputPath(package.relative_path.clone()))?;
    let json: serde_json::Value = serde_json::from_str(package_text)?;
    let dependencies = json.to_string();
    let selected = match framework {
        Framework::React => "@inertiajs/react",
        Framework::Svelte => "@inertiajs/svelte",
        Framework::Vue => "@inertiajs/vue3",
    };
    if !dependencies.contains(selected)
        || ["@inertiajs/react", "@inertiajs/svelte", "@inertiajs/vue3"]
            .into_iter()
            .filter(|adapter| *adapter != selected)
            .any(|adapter| dependencies.contains(adapter))
    {
        return Err(CliError::Message(
            "generated framework adapters are inconsistent".to_owned(),
        ));
    }
    let vite = plan
        .files
        .iter()
        .find(|file| file.relative_path == Path::new("vite.config.ts"))
        .ok_or_else(|| CliError::Message("missing vite.config.ts".to_owned()))?;
    let vite_text = std::str::from_utf8(&vite.contents)
        .map_err(|_| CliError::UnsafeOutputPath(vite.relative_path.clone()))?;
    if !vite_text.contains("rolldownOptions") || vite_text.contains("rollupOptions") {
        return Err(CliError::Message(
            "generated Vite configuration must use rolldownOptions".to_owned(),
        ));
    }
    Ok(())
}

/// Rejects output paths that could escape staging.
pub fn validate_relative_path(path: &Path) -> Result<(), CliError> {
    if path.as_os_str().is_empty()
        || path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        Err(CliError::UnsafeOutputPath(path.to_owned()))
    } else {
        Ok(())
    }
}
