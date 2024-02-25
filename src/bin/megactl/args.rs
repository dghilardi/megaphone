use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};

use megaphone::dto::agent::{AddVirtualAgentReqDto, PipeVirtualAgentReqDto};

/// Cli interface to port-plumber
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct PluCtlArgs {
    #[arg(short, long, default_value = "/run/megaphone.sock")]
    pub path: PathBuf,
    #[arg(short, long, default_value = "plain")]
    pub out_format: OutFormat,
    #[clap(subcommand)]
    pub subcommand: Commands,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum OutFormat {
    Plain,
    Json
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// List current virtual agents
    ListAgents,
    /// Register a new virtual agent
    AddAgent(AddAgentArgs),
    /// Pipe a virtual agent to a different megaphone instance
    PipeAgent(PipeAgentArgs),
    /// List active channels
    ListChannels(ListChannelsArgs),
    /// Terminate and remove a channel
    DisposeChannel(DisposeChannelArgs),
}

#[derive(Args, Debug)]
pub struct AddAgentArgs {
    #[arg(short, long)]
    pub name: String,
}

#[derive(Args, Debug)]
pub struct PipeAgentArgs {
    #[arg(short, long)]
    pub name: String,
    #[arg(short, long)]
    pub target: String,
}

impl From<AddAgentArgs> for AddVirtualAgentReqDto {
    fn from(value: AddAgentArgs) -> Self {
        Self {
            name: value.name,
        }
    }
}

impl From<PipeAgentArgs> for PipeVirtualAgentReqDto {
    fn from(value: PipeAgentArgs) -> Self {
        Self {
            name: value.name,
            target: value.target,
        }
    }
}

#[derive(Args, Debug)]
pub struct ListChannelsArgs {
    #[arg(short, long)]
    pub skip: Option<usize>,
    #[arg(short, long)]
    pub limit: Option<usize>,
}

#[derive(Args, Debug)]
pub struct DisposeChannelArgs {
    #[arg(short, long)]
    pub name: String,
}