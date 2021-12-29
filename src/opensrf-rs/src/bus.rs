use std::time;
use std::cmp;
use std::fmt;
use log::{trace, error};
use uuid::Uuid;
use redis;
use redis::Commands;
use super::conf::BusConfig;
use super::message::Message;

/// Each message sent to the bus will be chopped up into pieces no
/// larger than this.
///
/// The final chunk of each message will be given a trailing "\0"
/// character to indicate end-of-message.
const MSG_CHUNK_SIZE: usize = 1024;

/// Manages the Redis connection.
pub struct Bus {
    redis: Option<redis::Client>,
}

impl Bus {

    pub fn new() -> Self {
        Bus { redis: None }
    }

    /// Generates a unique address with a prefix string.
    pub fn new_addr(prefix: &str) -> String {
        let id = &Uuid::new_v4().to_simple().to_string();
        // Full UUID is overkill and a lot to look at in the logs.
        // Trim to first 16 chars.
        String::from(prefix) + "-" + &id[0..16]
    }

    /// Connect to the Redis instance.
    ///
    /// This must be called before any send/recv calls are made.
    pub fn connect(&mut self, bus_config: &BusConfig) -> Result<(), super::Error> {

        let dest = self.dest_str(bus_config)?;

        let client = match redis::Client::open(dest) {
            Ok(c) => c,
            Err(e) => { return Err(super::Error::BusError(e)); }
        };

        // Make sure we can get a handle on our connection before
        // we call this connect() a success.
        if let Err(e) = client.get_connection() {
            return Err(super::Error::BusError(e));
        }

        self.redis = Some(client);

        Ok(())
    }

    /// Generates the Redis connection URI
    pub fn dest_str(&self, bus_config: &BusConfig) -> Result<String, super::Error> {
        let dest: String;

        if let Some(ref s) = bus_config.sock() {
            dest = format!("unix://{}", s);
        } else {
            if let Some(ref h) = bus_config.host() {
                if let Some(ref p) = bus_config.port() {
                    dest = format!("redis://{}:{}/", h, p);
                } else {
                    error!("Bus requires 'sock' or 'host' + 'port'");
                    return Err(super::Error::ConfigError);
                }
            } else {
                error!("Bus requires 'sock' or 'host' + 'port'");
                return Err(super::Error::ConfigError);
            }
        };

        Ok(dest)
    }

    /// Get a handle on our internal Redis connection.
    fn connection(&self) -> Result<redis::Connection, super::Error> {
        match &self.redis {
            Some(r) => match r.get_connection() {
                Ok(c) => Ok(c),
                Err(e) => Err(super::Error::BusError(e))
            },
            None => Err(super::Error::InternalApiError("Bus::connect() call needed"))
        }
    }

    /// Returns at most one String pulled from the queue or None if the
    /// pop times out or is interrupted.
    ///
    /// The string will be valid JSON string, a partial JSON string, or
    /// a null character ("\0") indicating the end of a JSON string.
    fn recv_one_chunk(&self, sent_to: &str, timeout: i32) -> Result<Option<String>, super::Error> {

        trace!("recv_one_chunk() timeout={} for recipient {}", timeout, sent_to);

        let mut con = self.connection()?;

        let chunk: String;

        if timeout == 0 {

            // non-blocking
            // LPOP returns only the value
            chunk = match con.lpop(sent_to, None) {
                Ok(c) => c,
                Err(e) => match e.kind() {
                    redis::ErrorKind::TypeError => {
                        // Will read a Nil value on timeout.  That's OK.
                        return Ok(None);
                    },
                    _ => { return Err(super::Error::BusError(e)); }
                }
            };

        } else {

            // BLPOP returns the name of the popped list and the value.
            // This code assumes we're only recv()'ing for a single endpoint,
            // no wildcard matching on the sent_to.

            let resp: Vec<String>;

            if timeout < 0 { // block indefinitely

                resp = match con.blpop(sent_to, 0) {
                    Ok(r) => r,
                    Err(e) => { return Err(super::Error::BusError(e)); }
                };

            } else { // block up to timeout seconds

                resp = match con.blpop(sent_to, timeout as usize) {
                    Ok(r) => r,
                    Err(e) => { return Err(super::Error::BusError(e)); }
                };
            };

            if resp.len() == 0 { return Ok(None); } // No message received

            chunk = resp[1].to_string(); // resp = [key, value]
        }

        trace!("recv_one_chunk() pulled from queue: {}", chunk);

        Ok(Some(chunk))
    }

    /// Returns at most one JSON value pulled from the queue or None if
    /// the list pop times out or the pop is interrupted by a signal.
    fn recv_one_value(&self, sent_to: &str, timeout: i32) ->
        Result<Option<json::JsonValue>, super::Error> {

        let mut json_string: String = String::new();
        let mut len: usize;

        loop {

            match self.recv_one_chunk(sent_to, timeout)? {
                Some(chunk) => {
                    json_string += &chunk;
                    len = json_string.len();

                    // JSON streams are terminated with a null char and it
                    // will always be the last character in a chunk.
                    if &json_string[(len - 1)..] == "\0" { break; }
                },
                None => { return Ok(None); } // timeout exceeded w/ no data
            }
        }

        // Avoid parsing the trailing "\0"
        match json::parse(&json_string[..len - 1]) {
            Ok(json_val) => Ok(Some(json_val)),

            // Discard bad JSON and log the error instead of
            // bubbling it up to the caller.
            Err(err_msg) => {
                error!("Error parsing JSON: {:?}", err_msg);
                return Ok(None);
            }
        }
    }

    /// Returns at most one JSON value pulled from the queue.
    ///
    /// Keeps trying until a value is returned or the timeout is exceeded.
    ///
    /// # Arguments
    ///
    /// * `sent_to` - The message recipient (i.e. the Redis list key)
    /// * `timeout` - Time in seconds to wait for a value.
    ///     A negative value means to block indefinitely.
    ///     0 means do not block.
    pub fn recv_json_value(&self, sent_to: &str, timeout: i32) ->
        Result<Option<json::JsonValue>, super::Error> {

        let mut option: Option<json::JsonValue>;

        if timeout == 0 {

            // See if any data is ready now
            return self.recv_one_value(sent_to, timeout);

        } else if timeout < 0 {

            // Keep trying until we have a result.
            loop {
                option = self.recv_one_value(sent_to, timeout)?;
                if let Some(_) = option { return Ok(option); }
            }
        }

        // Keep trying until we have a result or exhaust the timeout.

        let mut seconds = timeout;

        while seconds > 0 {

            let now = time::SystemTime::now();

            option = self.recv_one_value(sent_to, timeout)?;

            match option {
                None => {
                    if seconds < 0 { return Ok(None); }
                    seconds -= now.elapsed().unwrap().as_secs() as i32;
                    continue;
                },
                _ => return Ok(option)
            }
        }

        Ok(None)
    }

    pub fn recv(&self, sent_to: &str, timeout: i32) -> Result<Option<Message>, super::Error> {

        let json_op = self.recv_json_value(sent_to, timeout)?;

        match json_op {
            Some(ref jv) => Ok(Message::from_json_value(jv)),
            None => Ok(None)
        }
    }

    pub fn send(&self, msg: &Message) -> Result<(), super::Error> {
        let recipient = msg.to();
        self.send_json_value(recipient, &msg.to_json_value())
    }

    /// Sends the JSON value as a JSON string to the recipient in chunks
    /// no larger than MSG_CHUNK_SIZE.
    pub fn send_json_value(&self, send_to: &str, value: &json::JsonValue) -> Result<(), super::Error> {

        let json_str = value.dump();
        let len = json_str.len();

        let mut start_pos: usize = 0;

        while start_pos < len {

            let chunk_size: usize = cmp::min(MSG_CHUNK_SIZE, len - start_pos);
            let end_pos = chunk_size + start_pos;
            let chunk = &json_str[start_pos..end_pos];

            if end_pos < len {

                self.rpush_wrapper(send_to, chunk)?;

            } else {
                // This is our final chunk.  Append the null char terminator.

                let with_term = String::from(chunk) + "\0";
                self.rpush_wrapper(send_to, &with_term)?;
                break;
            }

            start_pos = end_pos;
        }

        Ok(())
    }

    fn rpush_wrapper(&self, send_to: &str, text: &str) -> Result<(), super::Error> {

        let mut con = self.connection()?;

        trace!("send() writing chunk to={}: {}", send_to, text);

        let res: Result<i32, _> = con.rpush(send_to, text);

        if let Err(e) = res { return Err(super::Error::BusError(e)); }

        Ok(())
    }

    /// Clears the value for a key.
    pub fn clear(&self, key: &str) -> Result<(), super::Error> {

        let mut con = self.connection()?;

        let res: Result<i32, _> = con.del(key);

        match res {
            Ok(count) => trace!("con.del('{}') returned {}", key, count),
            Err(e) => { return Err(super::Error::BusError(e)) }
        }

        Ok(())
    }
}

impl fmt::Display for Bus {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Bus")
    }
}
