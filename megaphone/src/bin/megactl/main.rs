use clap::Parser;
use hyper::Uri;

use megaphone::dto::agent::{AddVirtualAgentReqDto, BasicOutcomeDto, PipeVirtualAgentReqDto, VirtualAgentItemDto};

use crate::args::{Commands, PluCtlArgs};
use crate::client::{SimpleRest, TcpSocketUri, UnixClient};
use crate::command::execute_command;

mod args;
mod client;
mod command;

#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    env_logger::init();
    let args: PluCtlArgs = PluCtlArgs::parse();

    let client = SimpleRest::from(UnixClient::new());
    match args.subcommand {
        Commands::ListAgents => {
            execute_command(
                args.out_format,
                || client.get::<_, Vec<VirtualAgentItemDto>>(TcpSocketUri::new(args.path, "/vagent/list")),
            ).await;
        }
        Commands::AddAgent(add_agent_args) => {
            execute_command(
                args.out_format,
                || client.post::<_, _, BasicOutcomeDto>(TcpSocketUri::new(args.path, "/vagent/add"), AddVirtualAgentReqDto::from(add_agent_args)),
            ).await;
        }
        Commands::PipeAgent(pipe_agent_args) => {
            execute_command(
                args.out_format,
                || client.post::<_, _, BasicOutcomeDto>(TcpSocketUri::new(args.path, "/vagent/pipe"), PipeVirtualAgentReqDto::from(pipe_agent_args)),
            ).await;
        },
        Commands::ListChannels(_list_channels_args) => {
            execute_command(
                args.out_format,
                || client.get::<_, BasicOutcomeDto>(TcpSocketUri::new(args.path, "/channel/list")),
            ).await;
        },
        Commands::DisposeChannel(dispose_channels_args) => {
            execute_command(
                args.out_format,
                || client.delete::<_, BasicOutcomeDto>(TcpSocketUri::new(args.path, &format!("/channel/{}", dispose_channels_args.name))),
            ).await;

        }
    }
    Ok(())
}