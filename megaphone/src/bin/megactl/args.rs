use std::net::SocketAddr;
use std::path::PathBuf;
use clap::{Args, Parser, Subcommand};
use megaphone::dto::agent::{AddVirtualAgentReqDto, PipeVirtualAgentReqDto};

/// Cli interface to port-plumber
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct PluCtlArgs {
    #[arg(short, long, default_value = "/run/megaphone.sock")]
    pub path: PathBuf,
    #[clap(subcommand)]
    pub subcommand: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// List current virtual agents
    ListAgents,
    /// Register a new virtual agent
    AddAgent(AddAgentArgs),
    /// Pipe a virtual agent to a different megaphone instance
    PipeAgent(PipeAgentArgs),
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
    pub target: SocketAddr,
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