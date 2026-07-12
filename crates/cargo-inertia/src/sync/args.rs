use clap::{Args, ValueEnum};
use std::path::PathBuf;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, ValueEnum)]
pub enum OutputLayout {
    #[default]
    Auto,
    Single,
    Modules,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, ValueEnum)]
pub enum LargeIntegerPolicy {
    #[default]
    Number,
    Error,
    Bigint,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, ValueEnum)]
pub enum DynamicPagePolicy {
    Ignore,
    #[default]
    Warn,
    Error,
}

#[derive(Args, Debug)]
#[allow(clippy::struct_excessive_bools)]
pub struct SyncArgs {
    #[arg(value_name = "OUTPUT", conflicts_with = "out")]
    pub output: Option<PathBuf>,
    #[arg(short = 'o', long = "out", value_name = "OUTPUT")]
    pub out: Option<PathBuf>,
    #[arg(long, default_value = ".")]
    pub path: PathBuf,
    #[arg(short = 'p', long = "package", conflicts_with = "workspace")]
    pub package: Option<String>,
    #[arg(long)]
    pub workspace: bool,
    #[arg(long, value_delimiter = ',')]
    pub features: Vec<String>,
    #[arg(long)]
    pub all_features: bool,
    #[arg(long)]
    pub no_default_features: bool,
    #[arg(long)]
    pub layout: Option<OutputLayout>,
    #[arg(long)]
    pub large_integers: Option<LargeIntegerPolicy>,
    #[arg(long)]
    pub dynamic_pages: Option<DynamicPagePolicy>,
    #[arg(long)]
    pub array_tuple_limit: Option<usize>,
    #[arg(long)]
    pub import_extension: Option<String>,
    #[arg(long)]
    pub check: bool,
    #[arg(long)]
    pub clean: bool,
    #[arg(long)]
    pub deny_warnings: bool,
    #[arg(long, conflicts_with_all = ["bin", "all_bins"])]
    pub lib: bool,
    #[arg(long, conflicts_with = "all_bins")]
    pub bin: Vec<String>,
    #[arg(long, conflicts_with = "bin")]
    pub all_bins: bool,
    #[arg(long)]
    pub examples: bool,
    #[arg(short, long)]
    pub verbose: bool,
}
impl SyncArgs {
    pub fn explicit_output(&self) -> Option<&PathBuf> {
        self.out.as_ref().or(self.output.as_ref())
    }
}
