use config::{Config, ConfigError, Environment, File};
use serde::Deserialize;

pub fn compose_config<'de, CFG: Deserialize<'de>>(external_path: &str, env_prefix: &str) -> Result<CFG, ConfigError> {
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
    pub agent_name: String,
}