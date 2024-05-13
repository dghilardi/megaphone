use std::future::Future;

use serde::Serialize;
use serde_json::json;

use megaphone::dto::agent::{BasicOutcomeDto, VirtualAgentItemDto};

use crate::args::OutFormat;

pub async fn execute_command<Cmd, FutRes, Res>(out_format: OutFormat, command: Cmd)
where
    Cmd: FnOnce() -> FutRes,
    FutRes: Future<Output = anyhow::Result<Res>>,
    Res: Printable,
{
    match (command().await, out_format) {
        (Ok(result), fmt) => result.print(fmt),
        (Err(error), OutFormat::Plain) => eprintln!("Error during command execution - {error}"),
        (Err(error), OutFormat::Json) => eprintln!(
            "{}",
            serde_json::to_string(&json!({ "out": "error", "message": error.to_string() }))
                .unwrap()
        ),
    }
}

pub trait Printable {
    fn print(&self, format: OutFormat);
}

impl<T> Printable for T
where
    T: PrintFormat<JsonFormat> + PrintFormat<PlainFormat>,
{
    fn print(&self, format: OutFormat) {
        match format {
            OutFormat::Plain => PrintFormat::<PlainFormat>::print(self),
            OutFormat::Json => PrintFormat::<JsonFormat>::print(self),
        }
    }
}

struct JsonFormat;
struct PlainFormat;
pub trait PrintFormat<F> {
    fn print(&self);
}

impl<S> PrintFormat<JsonFormat> for S
where
    S: Serialize,
{
    fn print(&self) {
        match serde_json::to_string(self) {
            Ok(serialized) => println!("{serialized}"),
            Err(err) => eprintln!("Error serializing result in json format - {err}"),
        }
    }
}

impl PrintFormat<PlainFormat> for Vec<VirtualAgentItemDto> {
    fn print(&self) {
        println!(
            "{0: <16} | {1: <6} | {2: <33} | {3: <10}",
            "NAME", "MODE", "SINCE", "CHANNELS"
        );
        for item in self {
            println!(
                "{0: <16} | {1: <10?} | {2: <33} | {3: <10}",
                item.name, item.mode, item.since, item.channels_count
            );
        }
    }
}

impl PrintFormat<PlainFormat> for BasicOutcomeDto {
    fn print(&self) {
        println!("Operation completed successfully");
    }
}
