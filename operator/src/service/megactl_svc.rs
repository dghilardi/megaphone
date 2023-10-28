use anyhow::anyhow;
use futures::StreamExt;
use k8s_openapi::api::core::v1::Pod;
use kube::Api;
use kube::api::AttachParams;
use serde::de::DeserializeOwned;
use megaphone::dto::agent::VirtualAgentItemDto;

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
        let exec_params = AttachParams::default().stderr(false);
        let command = ["/app/megactl", "-o", "json"].into_iter()
            .chain(args.into_iter().map(|a| a.as_ref()))
            .collect::<Vec<_>>();
        let mut cmd_out = self.pods_api.exec(pod_name, command, &exec_params).await?;
        let mut cmd_out_stream = tokio_util::io::ReaderStream::new(cmd_out.stdout().ok_or_else(|| anyhow!("Command returned empty output"))?);
        let next_stdout = cmd_out_stream.next().await
            .ok_or_else(|| anyhow!("Empty stdout"))??;
        let deserialized = serde_json::from_slice(&next_stdout)?;
        Ok(deserialized)
    }

    pub async fn list_agents(&self, pod: &str) -> anyhow::Result<Vec<VirtualAgentItemDto>> {
        self.exec_megactl(pod, &["list-agents"]).await
    }
}