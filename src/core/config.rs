use std::collections::HashMap;
use std::convert::Infallible;
use std::fmt;
use std::marker::PhantomData;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::str::FromStr;

use config::{Config, ConfigError, Environment, File};
use serde::de::{MapAccess, Visitor};
use serde::{de, Deserialize, Deserializer};

pub fn compose_config<'de, CFG: Deserialize<'de>>(
    external_path: &str,
    env_prefix: &str,
) -> Result<CFG, ConfigError> {
    Config::builder()
        // Add in a local configuration file
        .add_source(File::with_name(external_path).required(false))
        // Add in settings from the environment (with a prefix of CCS)
        .add_source(Environment::with_prefix(env_prefix))
        .build()?
        .try_deserialize()
}

#[derive(Deserialize)]
pub struct MegaphoneConfig {
    #[serde(default = "default_agent_warmup_secs")]
    pub agent_warmup_secs: u64,
    #[serde(default = "default_address")]
    pub address: SocketAddr,
    #[serde(default = "default_grpc_address")]
    pub grpc_address: SocketAddr,
    #[serde(default = "default_mng_socket_path")]
    pub mng_socket_path: PathBuf,
    #[serde(deserialize_with = "string_or_struct")]
    pub agent: AgentConfig,
    #[serde(default = "default_poll_duration")]
    pub poll_duration_millis: u64,
    #[serde(default)]
    pub webhooks: HashMap<String, WebHook>,
}

fn default_agent_warmup_secs() -> u64 {
    60
}

fn default_address() -> SocketAddr {
    "0.0.0.0:3000".parse().unwrap()
}
fn default_grpc_address() -> SocketAddr {
    "0.0.0.0:3001".parse().unwrap()
}

fn default_mng_socket_path() -> PathBuf {
    PathBuf::from("/run/megaphone.sock")
}

fn default_poll_duration() -> u64 {
    20_000
}

#[derive(Clone, Deserialize)]
pub struct WebHook {
    pub hook: WebHookType,
    pub endpoint: String,
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WebHookType {
    OnChannelDeleted,
}
#[derive(Clone, Deserialize)]
pub struct AgentConfig {
    #[serde(rename = "virtual")]
    pub virtual_agents: HashMap<String, VirtualAgentMode>,
}

impl FromStr for AgentConfig {
    type Err = Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self {
            virtual_agents: [(String::from(s), VirtualAgentMode::Master)]
                .into_iter()
                .collect(),
        })
    }
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum VirtualAgentMode {
    Master,
    Replica,
}

fn string_or_struct<'de, T, D>(deserializer: D) -> Result<T, D::Error>
where
    T: Deserialize<'de> + FromStr<Err = Infallible>,
    D: Deserializer<'de>,
{
    // This is a Visitor that forwards string types to T's `FromStr` impl and
    // forwards map types to T's `Deserialize` impl. The `PhantomData` is to
    // keep the compiler from complaining about T being an unused generic type
    // parameter. We need T in order to know the Value type for the Visitor
    // impl.
    struct StringOrStruct<T>(PhantomData<fn() -> T>);

    impl<'de, T> Visitor<'de> for StringOrStruct<T>
    where
        T: Deserialize<'de> + FromStr<Err = Infallible>,
    {
        type Value = T;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("string or map")
        }

        fn visit_str<E>(self, value: &str) -> Result<T, E>
        where
            E: de::Error,
        {
            Ok(FromStr::from_str(value).unwrap())
        }

        fn visit_map<M>(self, map: M) -> Result<T, M::Error>
        where
            M: MapAccess<'de>,
        {
            // `MapAccessDeserializer` is a wrapper that turns a `MapAccess`
            // into a `Deserializer`, allowing it to be used as the input to T's
            // `Deserialize` implementation. T then deserializes itself using
            // the entries from the map visitor.
            Deserialize::deserialize(de::value::MapAccessDeserializer::new(map))
        }
    }

    deserializer.deserialize_any(StringOrStruct(PhantomData))
}
