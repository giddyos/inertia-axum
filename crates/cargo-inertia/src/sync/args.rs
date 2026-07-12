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
    #[arg(long, default_value = "auto")]
    pub layout: OutputLayout,
    #[arg(long, default_value = "number")]
    pub large_integers: LargeIntegerPolicy,
    #[arg(long, default_value = "warn")]
    pub dynamic_pages: DynamicPagePolicy,
    #[arg(long, default_value_t = 64)]
    pub array_tuple_limit: usize,
    #[arg(long)]
    pub import_extension: Option<String>,
    #[arg(long)]
    pub check: bool,
    #[arg(long)]
    pub clean: bool,
    #[arg(long)]
    pub deny_warnings: bool,
    #[arg(long)]
    pub lib: bool,
    #[arg(long)]
    pub bin: Vec<String>,
    #[arg(long)]
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
