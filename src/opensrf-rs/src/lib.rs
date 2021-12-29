pub use self::error::Error;
pub use self::conf::ClientConfig;
/*
pub use self::bus::Bus;
pub use self::server::Server;
pub use self::method::Method;
pub use self::method::ParamCount;
pub use self::client::Client;
pub use self::worker::RequestContext;
pub use self::message::Request;
*/

mod error;
mod conf;
/*
mod bus;
mod message;
mod method;
mod worker;
mod server;
mod client;

#[cfg(test)]
mod tests;
*/

/*

/// Initialize logging and read the configuration file.
pub fn init(args: Vec<String>) -> Result<Config, self::error::Error> {

    env_logger::init();

    if args.len() < 2 {
        return Err(Error::NoConfigFileError);
    }

    let mut config = self::Config::new();
    config.load_file(&args[1])?;

    Ok(config)
}
*/


