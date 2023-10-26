use clap::Parser;
use hyper::Client;
use hyperlocal::{UnixClientExt, Uri};
use megaphone::dto::agent::{AddVirtualAgentReqDto, BasicOutcomeDto, PipeVirtualAgentReqDto, VirtualAgentItemDto};
use crate::args::{Commands, PluCtlArgs};
use crate::client::SimpleRest;

mod args;
mod client;

#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    let args: PluCtlArgs = PluCtlArgs::parse();

    let client = SimpleRest::from(Client::unix());
    match args.subcommand {
        Commands::ListAgents => {
            let res: Vec<VirtualAgentItemDto> = client.get(Uri::new(args.path, "/vagent/list")).await?;

            println!("{0: <16} | {1: <6} | {2: <10}", "NAME", "MODE", "SINCE");
            for item in res {
                println!("{0: <16} | {1: <10?} | {2: <10}", item.name, item.mode, item.since);
            }
        }
        Commands::AddAgent(add_agent_args) => {
            let opt_res: BasicOutcomeDto = client.post(Uri::new(args.path, "/vagent/add"), AddVirtualAgentReqDto::from(add_agent_args)).await?;
        }
        Commands::PipeAgent(pipe_agent_args) => {
            let opt_res: BasicOutcomeDto = client.post(Uri::new(args.path, "/vagent/pipe"), PipeVirtualAgentReqDto::from(pipe_agent_args)).await?;
        }
    }
    Ok(())
}