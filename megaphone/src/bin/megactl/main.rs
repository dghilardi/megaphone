use clap::Parser;
use hyper::Client;
use hyperlocal::{UnixClientExt, Uri};

use megaphone::dto::agent::{AddVirtualAgentReqDto, BasicOutcomeDto, PipeVirtualAgentReqDto, VirtualAgentItemDto};

use crate::args::{Commands, PluCtlArgs};
use crate::client::SimpleRest;
use crate::command::execute_command;

mod args;
mod client;
mod command;

#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    let args: PluCtlArgs = PluCtlArgs::parse();

    let client = SimpleRest::from(Client::unix());
    match args.subcommand {
        Commands::ListAgents => {
            execute_command(
                args.out_format,
                || client.get::<_, Vec<VirtualAgentItemDto>>(Uri::new(args.path, "/vagent/list")),
            ).await;
        }
        Commands::AddAgent(add_agent_args) => {
            execute_command(
                args.out_format,
                || client.post::<_, _, BasicOutcomeDto>(Uri::new(args.path, "/vagent/add"), AddVirtualAgentReqDto::from(add_agent_args)),
            ).await;
        }
        Commands::PipeAgent(pipe_agent_args) => {
            execute_command(
                args.out_format,
                || client.post::<_, _, BasicOutcomeDto>(Uri::new(args.path, "/vagent/pipe"), PipeVirtualAgentReqDto::from(pipe_agent_args)),
            ).await;
        }
    }
    Ok(())
}