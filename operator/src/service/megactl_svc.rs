use std::time::Duration;
use anyhow::{anyhow, bail, Context};
use futures::StreamExt;
use k8s_openapi::api::core::v1::Pod;
use kube::Api;
use kube::api::AttachParams;
use serde::de::DeserializeOwned;
use megaphone::dto::agent::{BasicOutcomeDto, VirtualAgentItemDto};

pub struct MegactlService {
    pods_api: Api<Pod>
}

impl MegactlService {

    pub fn new(
        pods_api: Api<Pod>,
    ) -> Self {
        Self {
            pods_api,
        }
    }
    async fn exec_megactl<Res: DeserializeOwned, Arg: AsRef<str>>(&self, pod_name: &str, args: &[Arg]) -> anyhow::Result<Res> {
        let exec_params = AttachParams::default().stderr(true);
        let command = ["/app/megactl", "-o", "json"].into_iter()
            .chain(args.into_iter().map(|a| a.as_ref()))
            .collect::<Vec<_>>();
        log::debug!("Running command '{}' on pod {pod_name}", command.join(" "));

        let mut cmd_out = self.pods_api.exec(pod_name, command, &exec_params).await?;
        let out_status = cmd_out.take_status().unwrap().await
            .ok_or_else(|| anyhow!("Cannot read cmd out status"))?;
        if out_status.status.as_ref().map(|status| status.eq("Success")).unwrap_or(false) {
            if let Some(stderr) = cmd_out.stderr() {
                let mut cmd_err_stream = tokio_util::io::ReaderStream::new(stderr);
                while let Some(Ok(next_stderr)) = cmd_err_stream.next().await {
                    log::error!("ERR: {}", String::from_utf8(next_stderr.to_vec()).unwrap_or_default())
                }
            }
            let mut cmd_out_stream = tokio_util::io::ReaderStream::new(cmd_out.stdout().ok_or_else(|| anyhow!("Command returned empty output"))?);
            let next_stdout = cmd_out_stream.next().await
                .ok_or_else(|| anyhow!("Empty stdout"))??;
            let deserialized = serde_json::from_slice(&next_stdout)?;
            Ok(deserialized)
        } else {
            bail!("Command exited with status {:?} - {}", out_status.status, out_status.message.unwrap_or_default())
        }
    }

    async fn exec_megactl_timeout<Res: DeserializeOwned, Arg: AsRef<str>>(&self, timeout: Duration, pod_name: &str, args: &[Arg]) -> anyhow::Result<Res> {
        tokio::time::timeout(timeout, self.exec_megactl(pod_name, args)).await
            .with_context(|| format!("megactl timeout for pod {pod_name}"))?
    }

    pub async fn list_agents(&self, pod: &str) -> anyhow::Result<Vec<VirtualAgentItemDto>> {
        self.exec_megactl_timeout(Duration::from_secs(30), pod, &["list-agents"]).await
    }

    pub async fn pipe_agent(&self, pod: &str, agent_name: &str, target_url: &str) -> anyhow::Result<BasicOutcomeDto> {
        self.exec_megactl_timeout(Duration::from_secs(30), pod, &["pipe-agent", "-n", agent_name, "-t", target_url]).await
    }
}