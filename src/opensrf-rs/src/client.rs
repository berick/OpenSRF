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
    pub fn session(&mut self, service: &str) -> &mut Session {
        let mut ses = Session::new(service);
        let thread = ses.thread().to_string();
        self.sessions.insert(thread.to_string(), ses);

        /// unwrap() OK here since we just inserted the value.
        self.get_session_mut(&thread).unwrap()
    }

    pub fn get_session(&self, thread: &str) -> Option<&Session> {
        self.sessions.get(thread)
    }

    pub fn get_session_mut(&mut self, thread: &str) -> Option<&mut Session> {
        self.sessions.get_mut(thread)
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

    /// Each new Request within a Session gets a new thread_trace.
    /// Replies have the same thread_trace as their request.
    thread_trace: u64,

    /// Though a TransportMessage may contain multiple messages within
    /// its body, our recv() call processes a single osrf message at
    /// a time.  Store the yet-to-be-processed ones here.  They'll be
    /// processed on subsequent calls to recv().
    pending_messages: Vec<message::Message>,
}

impl Session {

    fn new(service: &str) -> Self {
        Session {
            session_type: SessionType::Client,
            service: String::from(service),
            connected: false,
            remote_addr: None,
            thread: util::random_16(),
            thread_trace: 0,
            pending_messages: Vec::new(),
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
    */

/*
    /// Create and send a message::Message with a message::Request payload
    /// based on the provided method and params.
    ///
    /// The created message::Request is returned and may be used to
    /// receive responses.
    pub fn request(&mut self, method: &str,
        params: Vec<json::JsonValue>) -> Result<Request, error::Error> {

        self.thread_trace += 1;

        let thread_trace = self.thread_trace;

        let method = Payload::Method(Method::new(method, params));

        let req = Message::new(MessageType::Request, thread_trace, method);

        let mut tm = TransportMessage::new(
            self.remote_addr(), self.client.bus.bus_id(), &self.thread);

        tm.body_as_mut().push(req);

        self.client.bus.send(&tm)?;

        Ok(Request {
            thread_trace: thread_trace,
            complete: false,
        })
    }

    /// Returns the next reply from the client pending_messages queue matching
    /// this request if present.
    ///
    /// If a reply is found, it is removed from the queue.
    fn recv_from_queue(&mut self, thread_trace: u64) -> Option<message::Message> {

        if let Some(index) =
            self.pending_messages.iter().position(|m| m.thread_trace() == thread_trace) {

            trace!("Found a stashed reply for request {}", thread_trace);

            let msg = self.pending_messages.remove(index);

            Some(msg)

        } else {
            None
        }
    }

    /// Pull messages from the bus until we get to the first message
    /// that matches our thread, i.e. it's a response to this
    /// Session, or we exceed the timeout.
    pub fn pull_from_bus(&mut self, mut timeout: i32) ->
        Result<Option<TransportMessage>, error::Error> {

        loop {

            let start = time::SystemTime::now();

            let recv_op = self.client.bus.recv(timeout)?;

            if timeout > 0 {
                timeout -= start.elapsed().unwrap().as_secs() as i32;
            }

            let mut tm = match recv_op {
                Some(m) => m,
                None => { return Ok(None); } // Timed out
            };

            if tm.thread() == self.thread() {

                return Ok(Some(tm));

            } else {

                self.client.pending_transport_messages.push(tm);
            }
        }
    }


    /// Recieve a response for this request.
    ///
    /// First tries the client response queue, followed by the message
    /// bus.  Returns None on timeout or if this Request has already
    /// been marked as complete.
    pub fn recv(&mut self, req: &mut Request, timeout: i32)
        -> Result<Option<json::JsonValue>, error::Error> {

        // Request previously marked as complete
        if req.complete { return Ok(None); }

        // Reply for this request was previously pulled from the
        // bus and tossed into our queue.
        if let Some(msg) = self.recv_from_queue(req.thread_trace()) {
            match self.session_type {
                SessionType::Client => { return self.handle_client(&msg, req); },
                SessionType::Server => { return self.handle_server(&msg, req); },
            }
        }

        let mut tm = match self.pull_from_bus(timeout)? {
            Some(m) => m,
            None => { return Ok(None); } // Timeout
        };

        if self.remote_addr() != tm.from() {
            self.remote_addr = Some(tm.from().to_string());
        }

        if tm.body().len() == 0 { return Ok(None); }

        let msg = tm.body_as_mut().remove(0);

        let mut resp: Result<Option<json::JsonValue>, error::Error> = Ok(None);

        if msg.thread_trace() == req.thread_trace() {

            match self.session_type {
                SessionType::Client => resp = self.handle_client(&msg, req),
                SessionType::Server => resp = self.handle_server(&msg, req),
            }

        } else {
            /// Pulled a reply to a request made by this client session
            /// but not matching the requested thread trace.

            trace!("recv() found a reply for a request {}, stashing",
                msg.thread_trace());

            self.pending_messages.push(msg);
        }

        while tm.body().len() > 0 {
            // Our transport message has more messages in its body.
            // Toss them into the session queue so they'll be picked
            // up with the next call to recv();
            self.pending_messages.push(tm.body_as_mut().remove(0));
        }

        return resp;
    }

    fn handle_client(&mut self, msg: &message::Message, req: &mut Request)
        -> Result<Option<json::JsonValue>, error::Error> {

        if let Payload::Result(resp) = msg.payload() {
            // TODO check status code ?
            return Ok(Some(resp.content().clone()));
        };

        if let Payload::Status(stat) = msg.payload() {

            match stat.status() {
                MessageStatus::Ok => self.connected = true,
                MessageStatus::Continue => {}, // TODO
                MessageStatus::Complete => req.complete = true,
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

    fn handle_server(&mut self, msg: &message::Message, req: &mut Request)
        -> Result<Option<json::JsonValue>, error::Error> {

        /// TODO servers don't need a Request
        Ok(None)
    }

    */

}


pub struct Request {
    thread_trace: u64,
    complete: bool,
}

impl Request {
    pub fn thread_trace(&self) -> u64 {
        self.thread_trace
    }

    pub fn complete(&self) -> bool {
        self.complete
    }
}
