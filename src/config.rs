use serde::{Deserialize, Serialize};
use std::{fs::File, path::Path};

#[derive(Serialize, Deserialize, Debug)]
pub struct ServerInfo {
    pub addr: String,
    pub server_name: String,
    pub id: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct TlsConfigInfo {
    pub cert_path: String,
    pub key_path: String,
    pub ca_cert_path: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ServerConfig {
    pub addr: String,
    pub tls_config_info: TlsConfigInfo,
    pub ping: bool,
    pub id: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Config {
    pub me: ServerConfig,
    pub retry_delay: u32,
    pub servers: Vec<ServerInfo>,
}

impl Config {
    pub fn new(mut args: impl Iterator<Item = String>) -> Config {
        // skip executable
        args.next();

        let config_file_path = args.next().unwrap();

        let config_file_handle = File::open(Path::new(&config_file_path)).unwrap();
        let config: Config = serde_json::from_reader(config_file_handle).unwrap();
        config
    }
}
