use std::fs;
use log::{trace, debug, error};
use super::error::Error;

#[derive(Debug, Clone)]
pub struct BusConfig {
    host: Option<String>,
    port: Option<u16>,

    /// Unix Socket path
    sock: Option<String>,
}

impl BusConfig {

    pub fn new() -> Self {
        BusConfig {
            host: None,
            port: None,
            sock: None,
        }
    }

    pub fn host(&self) -> &Option<String> {
        &self.host
    }

    pub fn port(&self) -> &Option<u16> {
        &self.port
    }

    pub fn sock(&self) -> &Option<String> {
        &self.sock
    }

    pub fn set_host(&mut self, host: &str) {
        self.host = Some(String::from(host));
    }

    pub fn set_port(&mut self, port: u16) {
        self.port = Some(port);
    }

    pub fn set_sock(&mut self, sock: &str) {
        self.sock = Some(String::from(sock));
    }
}

#[derive(Debug, Clone)]
enum LogFile {
    Syslog,
    File(String),
}

#[derive(Debug, Clone)]
enum LogLevel {
    Error    = 1,
    Warning  = 2,
    Info     = 3,
    Debug    = 4,
    Internal = 5,
}

#[derive(Debug)]
pub struct ClientConfig {
    bus_config: BusConfig,
    log_file: LogFile,
    log_level: LogLevel,
    syslog_facility: Option<String>,
    actlog_facility: Option<String>,
    settings_file: Option<String>
}

impl ClientConfig {

    pub fn new() -> Self {
        ClientConfig {
            bus_config: BusConfig::new(),
            log_file: LogFile::Syslog,
            log_level: LogLevel::Info,
            syslog_facility: None,
            actlog_facility: None,
            settings_file: None,
        }
    }

    pub fn bus_config(&self) -> &BusConfig {
        &self.bus_config
    }

    /// Load configuration from an XML file
    pub fn load_file(&mut self, config_file: &str) -> Result<(), Error> {
        Ok(())
    }

    /// Load configuration from an XML string
    pub fn load_string(&mut self, xml: &str) -> Result<(), Error> {
        Ok(())
    }
}


