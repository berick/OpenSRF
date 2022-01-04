/*
pub use self::error;
pub use self::conf;
pub use self::message;
pub use self::message;
pub use self::bus::Bus;
pub use self::server::Server;
pub use self::method::Method;
pub use self::method::ParamCount;
pub use self::client::Client;
pub use self::worker::RequestContext;
pub use self::message::Request;
*/

pub mod error;
pub mod util;
pub mod conf;
pub mod classified;
pub mod message;
pub mod bus;
pub mod client;

#[cfg(test)]
mod tests;

/*
mod method;
mod worker;
mod server;

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


