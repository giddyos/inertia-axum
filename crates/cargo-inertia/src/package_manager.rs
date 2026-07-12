//! Package-manager selection, detection, and shell-free command planning.

use std::{ffi::OsString, path::PathBuf};

#[cfg(feature = "package-managers")]
use std::{fs, path::Path};

/// A JavaScript package manager supported by generated projects.
#[cfg_attr(feature = "cli", derive(clap::ValueEnum))]
#[cfg_attr(feature = "templates", derive(serde::Serialize))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PackageManager {
    /// npm.
    Npm,
    /// pnpm.
    Pnpm,
    /// Bun.
    Bun,
}

impl PackageManager {
    /// Returns the executable name for this manager.
    pub const fn executable(self) -> &'static str {
        match self {
            Self::Npm => "npm",
            Self::Pnpm => "pnpm",
            Self::Bun => "bun",
        }
    }

    /// Builds an installation command without invoking a shell.
    pub fn install_command(self, frontend: PathBuf) -> CommandSpec {
        CommandSpec {
            program: self.executable().into(),
            args: vec!["install".into()],
            current_dir: frontend,
        }
    }

    /// Builds a package-script command without invoking a shell.
    pub fn run_script(
        self,
        frontend: PathBuf,
        script: &str,
        forwarded: impl IntoIterator<Item = OsString>,
    ) -> CommandSpec {
        let mut args = match self {
            Self::Npm => vec!["run".into(), script.into(), "--".into()],
            Self::Pnpm | Self::Bun => vec!["run".into(), script.into()],
        };
        args.extend(forwarded);
        CommandSpec {
            program: self.executable().into(),
            args,
            current_dir: frontend,
        }
    }

    /// Finds the manager executable using the platform search path.
    #[cfg(all(feature = "package-managers", feature = "init"))]
    pub fn find_executable(self) -> Result<PathBuf, which::Error> {
        which::which(self.executable())
    }
}

/// An explicit package manager or deterministic automatic resolution.
#[cfg_attr(feature = "cli", derive(clap::ValueEnum))]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum PackageManagerChoice {
    /// Resolve from project metadata or installed executables.
    #[default]
    Auto,
    /// npm.
    Npm,
    /// pnpm.
    Pnpm,
    /// Bun.
    Bun,
}

impl From<PackageManager> for PackageManagerChoice {
    fn from(value: PackageManager) -> Self {
        match value {
            PackageManager::Npm => Self::Npm,
            PackageManager::Pnpm => Self::Pnpm,
            PackageManager::Bun => Self::Bun,
        }
    }
}

impl PackageManagerChoice {
    /// Returns the explicit manager, if the choice is not automatic.
    pub const fn explicit(self) -> Option<PackageManager> {
        match self {
            Self::Auto => None,
            Self::Npm => Some(PackageManager::Npm),
            Self::Pnpm => Some(PackageManager::Pnpm),
            Self::Bun => Some(PackageManager::Bun),
        }
    }
}

/// A process invocation represented as program and arguments, never a shell string.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommandSpec {
    /// The executable to launch.
    pub program: OsString,
    /// Arguments passed directly to the executable.
    pub args: Vec<OsString>,
    /// The directory in which to launch the process.
    pub current_dir: PathBuf,
}

/// Resolves a package manager using lockfiles, metadata, then installed executables.
#[cfg(feature = "package-managers")]
pub fn detect(
    choice: PackageManagerChoice,
    frontend: &Path,
    project_root: &Path,
    workspace_root: Option<&Path>,
) -> Result<PackageManager, DetectionError> {
    if let Some(manager) = choice.explicit() {
        return Ok(manager);
    }
    for directory in directories_to_search(frontend, project_root, workspace_root) {
        if let Some(manager) = manager_from_lockfiles(&directory)? {
            return Ok(manager);
        }
        if let Some(manager) = manager_from_package_json(&directory)? {
            return Ok(manager);
        }
    }
    [
        PackageManager::Npm,
        PackageManager::Pnpm,
        PackageManager::Bun,
    ]
    .into_iter()
    .find(|manager| manager.find_executable().is_ok())
    .ok_or(DetectionError::NoInstalledManager)
}

#[cfg(feature = "package-managers")]
fn directories_to_search(
    frontend: &Path,
    project_root: &Path,
    workspace_root: Option<&Path>,
) -> Vec<PathBuf> {
    let mut directories = vec![frontend.to_path_buf()];
    if project_root != frontend {
        directories.push(project_root.to_path_buf());
    }
    let stop = workspace_root.unwrap_or(project_root);
    let mut current = project_root.parent();
    while let Some(directory) = current {
        if directory == stop.parent().unwrap_or(directory) {
            break;
        }
        if !directories.iter().any(|item| item == directory) {
            directories.push(directory.to_path_buf());
        }
        if directory == stop {
            break;
        }
        current = directory.parent();
    }
    directories
}

#[cfg(feature = "package-managers")]
fn manager_from_lockfiles(directory: &Path) -> Result<Option<PackageManager>, DetectionError> {
    let groups = [
        (
            PackageManager::Npm,
            ["package-lock.json", "npm-shrinkwrap.json"].as_slice(),
        ),
        (PackageManager::Pnpm, ["pnpm-lock.yaml"].as_slice()),
        (PackageManager::Bun, ["bun.lock", "bun.lockb"].as_slice()),
    ];
    let found = groups
        .iter()
        .filter_map(|(manager, names)| {
            names
                .iter()
                .find(|name| directory.join(name).is_file())
                .map(|name| (*manager, directory.join(name)))
        })
        .collect::<Vec<_>>();
    match found.len() {
        0 => Ok(None),
        1 => Ok(Some(found[0].0)),
        _ => Err(DetectionError::ConflictingLockfiles {
            files: found.into_iter().map(|(_, path)| path).collect(),
        }),
    }
}

#[cfg(feature = "package-managers")]
fn manager_from_package_json(directory: &Path) -> Result<Option<PackageManager>, DetectionError> {
    let path = directory.join("package.json");
    if !path.is_file() {
        return Ok(None);
    }
    let text = fs::read_to_string(&path).map_err(DetectionError::Io)?;
    let Some(value) = package_manager_value(&text) else {
        return Ok(None);
    };
    let manager = value.split('@').next().unwrap_or_default();
    match manager {
        "npm" => Ok(Some(PackageManager::Npm)),
        "pnpm" => Ok(Some(PackageManager::Pnpm)),
        "bun" => Ok(Some(PackageManager::Bun)),
        "" => Ok(None),
        unsupported => Err(DetectionError::UnsupportedPackageManager {
            path,
            manager: unsupported.to_owned(),
        }),
    }
}

#[cfg(feature = "package-managers")]
fn package_manager_value(package_json: &str) -> Option<&str> {
    let key = "\"packageManager\"";
    let remainder = package_json.split_once(key)?.1;
    let remainder = remainder.trim_start().strip_prefix(':')?.trim_start();
    let remainder = remainder.strip_prefix('\"')?;
    remainder.split('\"').next()
}

/// A package-manager resolution failure.
#[derive(Debug)]
pub enum DetectionError {
    /// More than one supported lockfile exists in one directory.
    ConflictingLockfiles { files: Vec<PathBuf> },
    /// A project declares an unsupported package manager.
    UnsupportedPackageManager { path: PathBuf, manager: String },
    /// No supported executable was found on the search path.
    NoInstalledManager,
    /// Metadata could not be read.
    Io(std::io::Error),
}

impl std::fmt::Display for DetectionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ConflictingLockfiles { files } => write!(
                formatter,
                "Found conflicting package manager lockfiles:\n\n{}\n\nChoose one with --package-manager and remove the unused lockfile.",
                files
                    .iter()
                    .map(|path| format!("  {}", path.display()))
                    .collect::<Vec<_>>()
                    .join("\n")
            ),
            Self::UnsupportedPackageManager { manager, .. } => write!(
                formatter,
                "The project declares {manager}, but cargo-inertia supports npm, pnpm, and Bun."
            ),
            Self::NoInstalledManager => {
                write!(formatter, "no supported package manager is installed")
            }
            Self::Io(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for DetectionError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_specs_use_official_argument_conventions() {
        let frontend = PathBuf::from("frontend");
        assert_eq!(
            PackageManager::Npm
                .run_script(frontend.clone(), "dev", ["--host".into()])
                .args,
            ["run", "dev", "--", "--host"].map(OsString::from)
        );
        assert_eq!(
            PackageManager::Pnpm
                .run_script(frontend.clone(), "dev", ["--host".into()])
                .args,
            ["run", "dev", "--host"].map(OsString::from)
        );
        assert_eq!(
            PackageManager::Bun
                .run_script(frontend, "dev", ["--host".into()])
                .args,
            ["run", "dev", "--host"].map(OsString::from)
        );
    }

    #[cfg(all(feature = "package-managers", feature = "init"))]
    #[test]
    fn explicit_choice_bypasses_conflicting_lockfiles() {
        let directory = tempfile::tempdir().unwrap();
        fs::write(directory.path().join("package-lock.json"), "{}").unwrap();
        fs::write(
            directory.path().join("pnpm-lock.yaml"),
            "lockfileVersion: '9.0'",
        )
        .unwrap();
        assert_eq!(
            detect(
                PackageManagerChoice::Bun,
                directory.path(),
                directory.path(),
                None
            )
            .unwrap(),
            PackageManager::Bun
        );
    }

    #[cfg(feature = "package-managers")]
    #[test]
    fn conflicting_lockfiles_are_actionable() {
        let directory = tempfile::tempdir().unwrap();
        fs::write(directory.path().join("package-lock.json"), "{}").unwrap();
        fs::write(directory.path().join("bun.lock"), "{}").unwrap();
        assert!(matches!(
            detect(
                PackageManagerChoice::Auto,
                directory.path(),
                directory.path(),
                None
            ),
            Err(DetectionError::ConflictingLockfiles { .. })
        ));
    }
}
