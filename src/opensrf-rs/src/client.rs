use log::{trace, warn, error};
use std::collections::HashMap;
use std::time;
use super::*;
use super::message::TransportMessage;
use super::message::Message;
use super::message::MessageType;
use super::message::MessageStatus;
use super::message::Method;
use super::message::Payload;

pub struct Client {
    bus: bus::Bus,

    sessions: HashMap<String, Session>,

    /// Queue of receieved transport messages.
    /// Messages may arrive for one Session that are destined for
    /// another Session (i.e. different "thread" values).
    pending_transport_messages: Vec<message::TransportMessage>,
}

impl Client {

    pub fn new() -> Self {
        Client {
            bus: bus::Bus::new(bus::Bus::new_bus_id("client")),
            sessions: HashMap::new(),
            pending_transport_messages: Vec::new(),
        }
    }

    pub fn bus_connect(&mut self,
        bus_config: &conf::BusConfig) -> Result<(), error::Error> {
        self.bus.connect(bus_config)
    }

    pub fn bus_disconnect(&mut self) -> Result<(), error::Error> {
        // TODO redis::Client has no disconnect?
        // Does it happen automatically when the ref goes out of scope?
        self.bus.clear(&self.bus.bus_id())
    }

    /// Creates a new client session for communicating with the specified service.
    pub fn session(&mut self, service: &str) -> ClientSession {
        let mut ses = Session::new(service);

        let thread = ses.thread().to_string();

        self.sessions.insert(thread.to_string(), ses);

        ClientSession::new(self, thread)
    }

    /// Returns the first transport message pulled from the pending
    /// messages queue that matches the provided thread.
    fn recv_thread_from_queue(&mut self,
        thread: &str, mut timeout: i32) -> Option<TransportMessage> {

        if let Some(index) =
            self.pending_transport_messages.iter().position(|tm| tm.thread() == thread) {

            trace!("Found a stashed reply for thread {}", thread);

            let tm = self.pending_transport_messages.remove(index);

            Some(tm)

        } else {

            None
        }
    }

    /// Returns the first transport message pulled from the Bus that
    /// matches the provided thread.
    ///
    /// Messages that don't match the provided thread are pushed
    /// onto the pending transport message queue.
    pub fn recv_thread_from_bus(&mut self, thread: &str, mut timeout: i32) ->
        Result<Option<TransportMessage>, error::Error> {

        loop {

            let start = time::SystemTime::now();

            let recv_op = self.bus.recv(timeout)?;

            if timeout > 0 {
                timeout -= start.elapsed().unwrap().as_secs() as i32;
            }

            let mut tm = match recv_op {
                Some(m) => m,
                None => { return Ok(None); } // Timed out
            };

            if tm.thread() == thread {

                return Ok(Some(tm));

            } else {

                self.pending_transport_messages.push(tm);
            }
        }
    }

    /// Returns the first transport message pulled from either the
    /// pending messages queue or the bus.
    pub fn recv_thread(&mut self, thread: &str, mut timeout: i32) ->
        Result<Option<TransportMessage>, error::Error> {

        if let Some(tm) = self.recv_thread_from_queue(thread, timeout) {

            // We found a message destined for the requested thread
            // Update our Session so it has the latest remote_addr

            let mut ses = match self.sessions.get_mut(thread) {
                Some(s) => s,
                None => { return Err(error::Error::NoSuchThreadError); }
            };

            if ses.remote_addr() != tm.from() {
                ses.remote_addr = Some(tm.from().to_string());
            }

            Ok(Some(tm))
        } else {
            self.recv_thread_from_bus(thread, timeout)
        }
    }
}

pub enum SessionType {
    Client,
    Server,
}

/// Models a conversation with a single service.
pub struct Session {

    session_type: SessionType,

    connected: bool,

    /// Service name.
    ///
    /// For Clients, this doubles as the remote_addr when initiating
    /// a new conversation.
    /// For Servers, this is the name of the service we host.
    service: String,

    /// Worker-specific bus address for our session.
    /// Only set once we are communicating with a specific worker.
    remote_addr: Option<String>,

    /// Each session is identified by a random thread string.
    thread: String,
}

impl Session {

    fn new(service: &str) -> Self {
        Session {
            session_type: SessionType::Client,
            service: String::from(service),
            connected: false,
            remote_addr: None,
            thread: util::random_16(),
        }
    }

    fn thread(&self) -> &str {
        &self.thread
    }

    fn reset(&mut self) {
        self.remote_addr = None;
        self.thread = util::random_16();
    }

    fn remote_addr(&self) -> &str {
        if let Some(ref ra) = self.remote_addr {
            ra
        } else {
            &self.service
        }
    }
}

pub struct ClientSession<'cs> {
    client: &'cs mut Client,
    thread: String,

    /// Each new Request within a Session gets a new thread_trace.
    /// Replies have the same thread_trace as their request.
    thread_trace: u64,

    /// True if the most recent request has completed
    request_complete: bool,

    /// Though a TransportMessage may contain multiple messages within
    /// its body, our recv() call processes a single osrf message at
    /// a time.  Store the yet-to-be-processed ones here.  They'll be
    /// processed on subsequent calls to recv().
    pending_replies: Vec<message::Message>,
}

impl<'cs> ClientSession<'cs> {

    fn new(client: &'cs mut Client, thread: String) -> Self {
        ClientSession {
            client,
            thread,
            thread_trace: 0,
            request_complete: true,
            pending_replies: Vec::new(),
        }
    }

    pub fn thread(&self) -> &str {
        &self.thread
    }

    pub fn request_complete(&self) -> bool {
        self.request_complete
    }

    /// Create and send a message::Message with a message::Request payload
    /// based on the provided method and params.
    ///
    /// The created message::Request is returned and may be used to
    /// receive responses.
    pub fn request(&'cs mut self,
        method: &str, params: Vec<json::JsonValue>) -> Result<(), error::Error> {

        if !self.request_complete {
            return Err(error::Error::ActiveRequestError);
        }

        self.request_complete = false;

        self.thread_trace += 1;

        let ses = match self.client.sessions.get(self.thread()) {
            Some(s) => s,
            None => { return Err(error::Error::NoSuchThreadError); }
        };

        let method = Payload::Method(Method::new(method, params));

        let req = Message::new(MessageType::Request, self.thread_trace, method);

        let tm = TransportMessage::new_with_body(ses.remote_addr(),
            self.client.bus.bus_id(), self.thread(), req);

        self.client.bus.send(&tm)?;

        Ok(())
    }

    /// Returns the next reply from the client pending_replies queue matching
    /// this request if present.
    ///
    /// If a reply is found, it is removed from the queue.
    fn recv_from_pending(&mut self) -> Option<message::Message> {

        while self.pending_replies.len() > 0 {
            // Replies that do not match the current thread_trace are
            // likely lingering from previous requests.  Discard them.

            let msg = self.pending_replies.remove(0);

            if msg.thread_trace() == self.thread_trace {
                return Some(msg);
            }

        }

        return None
    }

    /// Recieve a response for this request.
    ///
    /// First tries the client response queue, followed by the message
    /// bus.  Returns None on timeout or if this Request has already
    /// been marked as complete.
    pub fn recv(&'cs mut self, timeout: i32) -> Result<Option<json::JsonValue>, error::Error> {

        // Request previously marked as complete
        if self.request_complete { return Ok(None); }

        // Reply for this request was previously pulled from the
        // bus and tossed into our queue.
        if let Some(msg) = self.recv_from_pending() {
            //return self.handle_client(&msg, req);
        }

        let thread = self.thread().to_string(); // TODO better way?

        let tm = match self.client.recv_thread(&thread, timeout)? {
        //let tm = match recv_thread_wrapped(&mut self.client, &thread, timeout)? {
            Some(m) => m,
            None => { return Ok(None); }
        };

        let mut msg_list = tm.body().to_owned();

        // We have received a message that matches this session's thread

        if msg_list.len() == 0 { return Ok(None); }

        let msg = msg_list.remove(0);

        let mut resp: Result<Option<json::JsonValue>, error::Error> = Ok(None);

        if msg.thread_trace() == self.thread_trace {

            // We have received a reply to the provided request.

            //resp = self.handle_reply(&msg);

        } else {

            /// Pulled a reply to a request made by this client session
            /// but not matching the requested thread trace.

            trace!("recv() found a reply for a request {}, stashing",
                msg.thread_trace());

            self.pending_replies.push(msg);
        }

        while msg_list.len() > 0 {

            // Our transport message has more messages in its body.
            // Toss them into the session queue so they'll be picked
            // up with the next call to recv();
            self.pending_replies.push(msg_list.remove(0));
        }

        return resp;
    }

    /*
    fn handle_reply(&mut self, msg: &message::Message, req: &mut Request)
        -> Result<Option<json::JsonValue>, error::Error> {

        if let Payload::Result(resp) = msg.payload() {
            // TODO check status code ?
            return Ok(Some(resp.content().clone()));
        };

        if let Payload::Status(stat) = msg.payload() {

            match stat.status() {
                MessageStatus::Ok => self.connected = true,
                MessageStatus::Continue => {}, // TODO
                MessageStatus::Complete => self.request_complete = true,
                MessageStatus::Timeout => {
                    self.reset();
                    warn!("Stateful session ended by server on keepalive timeout");
                    return Err(error::Error::RequestTimeoutError);
                },
                _ => {
                    self.reset();
                    warn!("Unexpected response status {}", stat.status());
                    return Err(error::Error::RequestTimeoutError);
                }
            }

            return Ok(None);
        }

        error!("Request::recv() unexpected response {}", msg.to_json_value().dump());

        self.reset();

        return Err(error::Error::BadResponseError);
    }
    */


    /*
    pub fn disconnect(&mut self) -> Result<(), error::Error> {

        if self.connect {
            let msg = message::Message::new(
                &self.remote_addr,
                &self.client.addr,
                0, // thread_trace
                message::MessageType::Disconnect,
                message::Payload::NoPayload,
            );

            self.client.bus.send(&msg)?;
        }

        // Avoid changing remote_addr until above message is composed.
        self.reset();

        Ok(())
    }

    pub fn respond(&mut self,
        req: &message::Request, value: &json::JsonValue) -> Result<(), error::Error> {

        let msg = message::Message::new(
            &self.remote_addr,
            &self.client.addr,
            req.thread_trace(),
            message::MessageType::Response,
            message::Payload::Response(message::Response::new(value.clone()))
        );

        self.client.bus.send(&msg)
    }

    fn handle_server(&mut self, msg: &message::Message, req: &mut Request)
        -> Result<Option<json::JsonValue>, error::Error> {

        /// TODO servers don't need a Request
        Ok(None)
    }
    */
}


