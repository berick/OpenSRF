use log::{trace, warn, error};
use std::time;
use super::*;
use super::message::TransportMessage;
use super::message::Message;
use super::message::MessageType;
use super::message::MessageStatus;
use super::message::Method;
use super::message::Payload;

const CONNECT_TIMEOUT: i32 = 10;

pub struct Client {
    bus: bus::Bus,

    sessions: Vec<Session>,

    /// Queue of receieved transport messages.
    /// Messages may arrive for one Session that are destined for
    /// another Session (i.e. different "thread" values).
    pending_transport_messages: Vec<message::TransportMessage>,
}

impl Client {

    pub fn new() -> Self {
        Client {
            bus: bus::Bus::new(bus::Bus::new_bus_id("client")),
            sessions: Vec::new(),
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

            trace!("recv_thread_from_bus() read thread = {}", tm.thread());

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

        let tm = match self.recv_thread_from_queue(thread, timeout) {
            Some(t) => t,
            None => {
                match self.recv_thread_from_bus(thread, timeout)? {
                    Some(t2) => t2,
                    None => { return Ok(None); }
                }
            }
        };

        let ses = self.get_session_mut(thread)?;

        if ses.remote_addr() != tm.from() {
            ses.remote_addr = Some(tm.from().to_string());
        }

        Ok(Some(tm))
    }

    /// Returns the next reply from the client pending_replies queue matching
    /// this request if present.
    ///
    /// If a reply is found, it is removed from the queue.
    fn recv_reply_from_backlog(&mut self, thread: &str) -> Result<Option<message::Message>, error::Error> {

        let ses = self.get_session_mut(thread)?;

        while ses.pending_replies.len() > 0 {

            // Replies that do not match the current last_thread_trace are
            // likely lingering from previous requests.  Discard them.
            let msg = ses.pending_replies.remove(0);

            if msg.thread_trace() == ses.last_thread_trace {

                trace!("recv_from_pending() found message trace={} type={}",
                    msg.thread_trace(), msg.mtype());

                return Ok(Some(msg));
            }
        }

        return Ok(None)
    }

    fn recv_reply_from_client(&mut self, thread: &str, 
        timeout: i32) -> Result<Option<json::JsonValue>, error::Error> {

        let last_thread_trace = self.get_session(thread)?.last_thread_trace;

        let tm = match self.recv_thread(thread, timeout)? {
            Some(m) => m,
            None => { return Ok(None); }
        };

        let mut msg_list = tm.body().to_owned();

        // We have received a message that matches this session's thread

        if msg_list.len() == 0 { return Ok(None); }

        let msg = msg_list.remove(0);

        let mut resp: Result<Option<json::JsonValue>, error::Error> = Ok(None);

        if msg.thread_trace() == last_thread_trace {

            // We have received a reply to the provided request.

            resp = self.handle_reply(thread, &msg);

        } else {

            /// Pulled a reply to a request made by this client session
            /// but not matching the requested thread trace.

            trace!("recv() found a reply for a request {}, stashing",
                msg.thread_trace());

            self.get_session_mut(thread)?.pending_replies.push(msg);
        }

        while msg_list.len() > 0 {

            trace!("Pushing OSRF message into pending replies trace={} mtype={}", 
                msg_list[0].thread_trace(), msg_list[0].mtype());

            // Our transport message has more messages in its body.
            // Toss them into the session queue so they'll be picked
            // up with the next call to recv();
            self.get_session_mut(thread)?.pending_replies.push(msg_list.remove(0));
        }

        return resp;
    }

    /// Recieve a response for a session
    ///
    /// First tries the client response queue, followed by the message
    /// bus.  Returns None on timeout or if this Request has already
    /// been marked as complete.
    pub fn recv(&mut self, thread: &str, 
        timeout: i32) -> Result<Option<json::JsonValue>, error::Error> {

        if self.get_session(thread)?.request_complete {
            // Latest request for session previously marked as complete
            return Ok(None);
        }

        if let Some(msg) = self.recv_reply_from_backlog(thread)? {
            // We already have a reply to this request in the queue.
            return self.handle_reply(thread, &msg);
        }

        self.recv_reply_from_client(thread, timeout)
    }

    fn handle_reply(&mut self, thread: &str, msg: &message::Message)
        -> Result<Option<json::JsonValue>, error::Error> {

        if let Payload::Result(resp) = msg.payload() {
            return Ok(Some(resp.content().clone()));
        };

        let ses = self.get_session_mut(thread)?;

        if let Payload::Status(stat) = msg.payload() {

            match stat.status() {
                MessageStatus::Ok => ses.connected = true,
                MessageStatus::Continue => {}, // TODO
                MessageStatus::Complete => {
                    ses.request_complete = true;
                    ses.reset();
                },
                MessageStatus::Timeout => {
                    ses.reset();
                    warn!("Stateful session ended by server on keepalive timeout");
                    return Err(error::Error::RequestTimeoutError);
                },
                _ => {
                    ses.reset();
                    warn!("Unexpected response status {}", stat.status());
                    return Err(error::Error::RequestTimeoutError);
                }
            }

            return Ok(None);
        }

        error!("Request::recv() unexpected response {}", msg.to_json_value().dump());

        ses.reset();

        return Err(error::Error::BadResponseError);
    }

    /// Creates a new client session for communicating with the specified service.
    pub fn session(&mut self, service: &str) -> String {
        let ses = Session::new(service);

        let thread = ses.thread().to_string();

        self.sessions.push(ses);

        thread
    }

    fn get_session(&self, thread: &str) -> Result<&Session, error::Error> {
        if let Some(index) =
            self.sessions.iter().position(|ses| &ses.thread == thread) {
            Ok(&self.sessions[index])
        } else {
            Err(error::Error::NoSuchThreadError)
        }
    }

    fn get_session_mut(&mut self, thread: &str) -> Result<&mut Session, error::Error> {
        if let Some(index) =
            self.sessions.iter().position(|ses| &ses.thread == thread) {
            Ok(&mut self.sessions[index])
        } else {
            Err(error::Error::NoSuchThreadError)
        }
    }

    pub fn request_is_complete(&self, thread: &str) -> Result<bool, error::Error> {
        Ok(self.get_session(thread)?.request_complete)
    }

    /// Create and send a message::Message with a message::Request payload
    /// based on the provided method and params.
    ///
    /// The created message::Request is returned and may be used to
    /// receive responses.
    pub fn request(&mut self, thread: &str, method: &str, 
        params: Vec<json::JsonValue>) -> Result<(), error::Error> {

        let bus_id = self.bus.bus_id().to_string(); // 

        let ses = self.get_session_mut(thread)?;

        if !ses.request_complete {
            return Err(error::Error::ActiveRequestError);
        }

        ses.request_complete = false;

        ses.last_thread_trace += 1;

        let method = Payload::Method(Method::new(method, params));

        let req = Message::new(MessageType::Request, ses.last_thread_trace, method);

        let tm = TransportMessage::new_with_body(
            ses.remote_addr(), &bus_id, thread, req);

        self.bus.send(&tm)
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

    /// Each new Request within a Session gets a new last_thread_trace.
    /// Replies have the same last_thread_trace as their request.
    last_thread_trace: u64,

    /// True if the most recent request has completed
    request_complete: bool,

    /// Though a TransportMessage may contain multiple messages within
    /// its body, our recv() call processes a single osrf message at
    /// a time.  Store the yet-to-be-processed ones here.  They'll be
    /// processed on subsequent calls to recv().
    pending_replies: Vec<message::Message>,
}

impl Session {

    fn new(service: &str) -> Self {
        Session {
            session_type: SessionType::Client,
            service: String::from(service),
            connected: false,
            remote_addr: None,
            thread: util::random_16(),
            last_thread_trace: 0,
            request_complete: true,
            pending_replies: Vec::new(),
        }
    }

    fn thread(&self) -> &str {
        &self.thread
    }

    fn reset(&mut self) {
        self.connected = false;
        self.remote_addr = Some(self.service.to_string());
        self.thread = util::random_16();
        self.pending_replies.clear();
    }

    fn remote_addr(&self) -> &str {
        if let Some(ref ra) = self.remote_addr {
            ra
        } else {
            &self.service
        }
    }

    /*
    fn send_connect(&mut self, service: &str, thread: &str) -> Result<(), error::Error> {

        if !self.request_complete {
            return Err(error::Error::ActiveRequestError);
        }

        self.last_thread_trace += 1;

        let msg = Message::new(
            MessageType::Connect, self.last_thread_trace, Payload::NoPayload);

        let tm = TransportMessage::new_with_body(
            service, self.client.bus.bus_id(), &thread, msg);

        self.client.bus.send(&tm)
    }

    pub fn connect(&mut self) -> Result<(), error::Error> {

        let thread = self.thread().to_string(); // TODO 

        let ses = match self.client.sessions.get(&thread) {
            Some(s) => s,
            None => { return Err(error::Error::NoSuchThreadError); }
        };

        self.send_connect(&thread, &ses.service)?;

        let mut timeout = CONNECT_TIMEOUT;

        while timeout > 0 { 

            let start = time::SystemTime::now();

            let recv_op = self.recv(timeout)?;

            if ses.connected { break; }

            timeout -= start.elapsed().unwrap().as_secs() as i32;
        }

        if ses.connected {
            Ok(())
        } else {
            Err(error::Error::ConnectTimeoutError)
        }
    }

    pub fn disconnect(&mut self) -> Result<(), error::Error> {

        let thread = self.thread().to_string(); // TODO 

        let mut ses = match self.client.sessions.get_mut(&thread) {
            Some(s) => s,
            None => { return Err(error::Error::NoSuchThreadError); }
        };

        if ses.connected {

            let msg = Message::new(
                MessageType::Disconnect, self.last_thread_trace, Payload::NoPayload);

            let tm = TransportMessage::new_with_body(ses.remote_addr(),
                self.client.bus.bus_id(), &thread, msg);

            self.client.bus.send(&tm)?;
        }

        // Avoid changing remote_addr until above message is composed.
        ses.reset();

        Ok(())
    }

    pub fn respond(&mut self,
        req: &message::Request, value: &json::JsonValue) -> Result<(), error::Error> {

        let msg = message::Message::new(
            &self.remote_addr,
            &self.client.addr,
            req.last_thread_trace(),
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


