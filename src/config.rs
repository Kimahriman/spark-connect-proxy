use std::collections::HashMap;

use figment::{
    providers::{Format, Json},
    Figment,
};
use local_ip_address::local_ip;
use serde::Deserialize;

const DEFAULT_PORT: u16 = 8100;

#[derive(Clone, Default, Deserialize)]
pub struct SparkVersion {
    // Name shown to users
    pub name: String,
    // SPARK_HOME directory for this version
    pub home: String,
    pub default: bool,
    pub env: Option<HashMap<String, String>>,
    pub default_configs: Option<HashMap<String, String>>,
    pub merge_configs: Option<HashMap<String, String>>,
    pub override_configs: Option<HashMap<String, String>>,
}

#[derive(Deserialize)]
pub struct TlsConfig {
    pub key: String,
    pub cert: String,
}

#[derive(Deserialize, Default)]
pub struct ProxyConfig {
    pub bind_host: Option<String>,
    pub bind_port: Option<u16>,
    pub callback_address: Option<String>,
    pub tls: Option<TlsConfig>,
    pub spark_versions: Vec<SparkVersion>,
}

impl ProxyConfig {
    pub fn from_file(path: impl AsRef<str>) -> Self {
        Figment::new()
            .merge(Json::file(path.as_ref()))
            .extract()
            .unwrap()
    }

    pub fn get_bind_port(&self) -> u16 {
        self.bind_port.unwrap_or(DEFAULT_PORT)
    }

    pub fn get_callback_addr(&self) -> String {
        self.callback_address.clone().unwrap_or_else(|| {
            let callback_scheme = if self.tls.is_some() { "https" } else { "http" };
            format!(
                "{}://{}:{}",
                callback_scheme,
                local_ip().unwrap(),
                self.get_bind_port()
            )
        })
    }

    // pub fn spark_versions(&self) -> Vec<SparkVersion> {
    //     self.config.get_table(key)
    // }
}
