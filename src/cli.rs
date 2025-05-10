use clap::Parser;

#[derive(Parser, Debug)]
#[clap(author="Huchen", version="0.1.0", about="Enhances Git with AI support", long_about=None)]
pub struct Cli {
    #[clap(subcommand)]
    pub command: GitCommands,
}

#[derive(Parser, Debug)]
pub enum GitCommands {
    /// Handle git commit operation
    Commit(CommitArgs),
    // Future: Add(AddArgs)
    // Future: Config(ConfigArgs)
}

#[derive(Parser, Debug)]
pub struct CommitArgs {
    /// Use AI to generate the commit message
    #[clap(long)]
    pub ai: bool,

    /// Pass a message to the commit
    #[clap(short, long)]
    pub message: Option<String>,

    /// Allow all other flags and arguments to be passed through to git commit
    #[clap(allow_hyphen_values = true, last = true)]
    pub passthrough_args: Vec<String>,
}