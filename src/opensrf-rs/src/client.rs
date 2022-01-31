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
use super::session::Request;
use super::session::Session;
use super::session::SessionType;

const CONNECT_TIMEOUT: i32 = 10;

// Immutable context structs the caller owns for managing
// sessions and requests.  These link to mutable variants
// internally so we don't have to bandy about mutable refs.
pub struct ClientSession {
    pub session_id: usize,
}
pub struct ClientRequest {
    pub session_id: usize,
    pub thread_trace: usize,
}

pub struct Client {
    bus: bus::Bus,

    sessions: HashMap<usize, Session>,

    /// Queue of receieved transport messages that have yet to be
    /// processed by any sessions.
    transport_backlog: Vec<message::TransportMessage>,

    last_session_id: usize,
}

impl Client {

    pub fn new() -> Self {
        Client {
            bus: bus::Bus::new(bus::Bus::new_bus_id("client")),
            sessions: HashMap::new(),
            transport_backlog: Vec::new(),
            last_session_id: 0,
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

    fn get_ses_by_thread(&self, thread: &str) -> Option<&Session> {

        if let Some(index) =
            self.sessions.values().position(|ses| &ses.thread == thread) {
            Some(self.sessions.get(&index));
        }

        None
    }

    fn get_ses_by_thread_mut(&mut self, thread: &str) -> Option<&mut Session> {
        if let Some(index) =
            self.sessions.values().position(|ses| &ses.thread == thread) {
            Some(self.sessions.get_mut(&index));
        }

        None
    }

    fn ses(&self, session_id: usize) -> &Session {
        self.sessions.get(&session_id).unwrap()
    }

    fn ses_mut(&mut self, session_id: usize) -> &mut Session {
        self.sessions.get_mut(&session_id).unwrap()
    }

    fn req(&self, req: &ClientRequest) -> &Request {
        self.ses(req.session_id).requests.get(&req.thread_trace).unwrap()
    }

    fn req_mut(&mut self, req: &ClientRequest) -> &mut Request {
        self.ses_mut(req.session_id).requests.get_mut(&req.thread_trace).unwrap()
    }

    pub fn session(&mut self, service: &str) -> ClientSession {
        self.last_session_id += 1;
        let session_id = self.last_session_id;

        let ses = Session::new(service, session_id);
        self.sessions.insert(session_id, ses);

        ClientSession {
            session_id: session_id
        }
    }

    /// Returns the first transport message pulled from the pending
    /// messages queue that matches the provided thread.
    fn recv_session_from_backlog(&mut self,
        thread: &str, mut timeout: i32) -> Option<TransportMessage> {

        if let Some(index) =
            self.transport_backlog.iter().position(|tm| tm.thread() == thread) {

            trace!("Found a stashed reply for thread {}", thread);

            Some(self.transport_backlog.remove(index))

        } else {

            None
        }
    }

    /// Returns the first transport message pulled from the Bus that
    /// matches the provided thread.
    ///
    /// Messages that don't match the provided thread are pushed
    /// onto the pending transport message queue.
    pub fn recv_session_from_bus(&mut self, thread: &str, mut timeout: i32) ->
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

            trace!("recv_session_from_bus() read thread = {}", tm.thread());

            if tm.thread() == thread {

                return Ok(Some(tm));

            } else {

                self.transport_backlog.push(tm);
            }
        }
    }

    /// Returns the first transport message pulled from either the
    /// backlog or the bus.
    pub fn recv_session(&mut self, session_id: usize, mut timeout: i32) ->
        Result<Option<TransportMessage>, error::Error> {

        let thread = self.ses(session_id).thread.to_string();

        trace!("recv_session() with thread={} timeout={}", thread, timeout);

        let tm = match self.recv_session_from_backlog(&thread, timeout) {
            Some(t) => t,
            None => {
                match self.recv_session_from_bus(&thread, timeout)? {
                    Some(t2) => t2,
                    None => { return Ok(None); }
                }
            }
        };

        let ses = self.ses_mut(session_id);

        if ses.remote_addr() != tm.from() {
            ses.remote_addr = Some(tm.from().to_string());
        }

        Ok(Some(tm))
    }

    pub fn cleanup(&mut self, session: &ClientSession) {
        self.sessions.remove(&session.session_id);
    }

    pub fn connect(&mut self, session: &ClientSession) -> Result<(), error::Error> {

        let session_id = session.session_id;
        let mut ses = self.sessions.get_mut(&session_id).unwrap();
        ses.last_thread_trace += 1;

        let thread_trace = ses.last_thread_trace;
        let thread = ses.thread.to_string();
        let remote_addr = ses.remote_addr().to_string();

        ses.requests.insert(thread_trace,
            Request {
                complete: false,
                session_id: session_id,
                thread_trace: thread_trace,
            }
        );

        let req = ClientRequest {
            session_id: session_id,
            thread_trace: thread_trace,
        };

        let msg = Message::new(MessageType::Connect,
            thread_trace, message::Payload::NoPayload);

        let tm = TransportMessage::new_with_body(
            &remote_addr, self.bus.bus_id(), &thread, msg);

        self.bus.send(&tm)?;

        let mut timeout = CONNECT_TIMEOUT;

        while timeout > 0 {

            let start = time::SystemTime::now();

            trace!("connect() calling receive with timeout={}", timeout);
            let recv_op = self.recv(&req, timeout)?;

            let ses = self.ses_mut(session_id);
            if ses.connected { return Ok(()) }

            timeout -= start.elapsed().unwrap().as_secs() as i32;
        }

        Err(error::Error::ConnectTimeoutError)
    }

    pub fn disconnect(&mut self, session: &ClientSession) -> Result<(), error::Error> {

        let session_id = session.session_id;

        // Disconnects need a thread trace, but no request ID, since
        // we do not track them internally -- they produce no response.
        self.ses_mut(session_id).last_thread_trace += 1;

        let ses = self.ses(session_id);

        let msg = Message::new(MessageType::Disconnect,
            ses.last_thread_trace, message::Payload::NoPayload);

        let tm = TransportMessage::new_with_body(
            &ses.remote_addr(), self.bus.bus_id(), &ses.thread, msg);

        self.bus.send(&tm)?;

        // Avoid changing remote_addr until above message is composed.
        self.ses_mut(session_id).reset();

        Ok(())
    }

    pub fn request(
        &mut self,
        session: &ClientSession,
        method: &str,
        params: Vec<json::JsonValue>) -> Result<ClientRequest, error::Error> {

        let session_id = session.session_id;
        let mut ses = self.sessions.get_mut(&session_id).unwrap();

        ses.last_thread_trace += 1;

        let method = Payload::Method(Method::new(method, params));

        let req = Message::new(MessageType::Request, ses.last_thread_trace, method);

        // If we're not connected, all requets go to the root service address.
        let remote_addr = match ses.connected {
            true => ses.remote_addr(),
            false => &ses.service
        };

        let tm = TransportMessage::new_with_body(
            remote_addr, self.bus.bus_id(), &ses.thread, req);

        self.bus.send(&tm)?;

        let mut ses = self.sessions.get_mut(&session_id).unwrap();

        trace!("request() session id={} adding request tt={}",
            session_id, ses.last_thread_trace);

        ses.requests.insert(ses.last_thread_trace,
            Request {
                complete: false,
                session_id: session_id,
                thread_trace: ses.last_thread_trace,
            }
        );

        Ok(ClientRequest {
            session_id: session_id,
            thread_trace: ses.last_thread_trace
        })
    }

    fn recv_from_backlog(&mut self, req: &ClientRequest) -> Option<message::Message> {
        let ses = self.ses_mut(req.session_id);

        trace!("recv_from_backlog() tt={}", req.thread_trace);

        match ses.backlog.iter().position(|resp| resp.thread_trace() == req.thread_trace) {
            Some(index) => {
                trace!("recv_from_backlog() found response for request tt={}", req.thread_trace);
                return Some(ses.backlog.remove(index));
            }
            None => None,
        }
    }

    // TODO loop on timeout until a response for the provided request
    // id arrives (or times out).
    pub fn recv(
        &mut self,
        req: &ClientRequest,
        mut timeout: i32) -> Result<Option<json::JsonValue>, error::Error> {

        let mut resp: Result<Option<json::JsonValue>, error::Error> = Ok(None);

        if self.complete(req) { return resp; }

        let session_id = req.session_id;
        let thread_trace = req.thread_trace;

        trace!("recv() thread_trace={}", thread_trace);

        // Reply for this request was previously pulled from the
        // bus and tossed into our queue.
        if let Some(msg) = self.recv_from_backlog(req) {
            return self.handle_reply(req, &msg);
        }

        while timeout >= 0 {

            let start = time::SystemTime::now();

            // Could return a response to any request that's linked
            // to this session.
            let tm = match self.recv_session(session_id, timeout)? {
                Some(m) => m,
                None => { return resp; }
            };

            timeout -= start.elapsed().unwrap().as_secs() as i32;

            let mut msg_list = tm.body().to_owned();

            if msg_list.len() == 0 { continue; }

            let msg = msg_list.remove(0);

            let mut found = false;
            if msg.thread_trace() == thread_trace {

                found = true;
                resp = self.handle_reply(req, &msg);

            } else {

                /// Pulled a reply to a request made by this client session
                /// but not matching the requested thread trace.

                trace!("recv() found a reply for a request {}, stashing",
                    msg.thread_trace());

                self.sessions.get_mut(&session_id).unwrap().backlog.push(msg);
            }

            while msg_list.len() > 0 {
                trace!("recv() adding to session backlog thread_trace={}",
                    msg_list[0].thread_trace());

                self.sessions.get_mut(&session_id).unwrap().backlog.push(
                    msg_list.remove(0)
                );
            }

            if found { break; }
        }

        return resp;
    }

    fn handle_reply(
        &mut self,
        req: &ClientRequest,
        msg: &message::Message
    ) -> Result<Option<json::JsonValue>, error::Error> {

        let session_id = req.session_id;

        if let Payload::Result(resp) = msg.payload() {
            trace!("handle_reply() found response for tt={}", req.thread_trace);
            return Ok(Some(resp.content().clone()));
        };

        if let Payload::Status(stat) = msg.payload() {

            match stat.status() {
                MessageStatus::Ok => {
                    trace!("handle_reply() marking session {} as connected", session_id);
                    self.sessions.get_mut(&session_id).unwrap().connected = true;
                },
                MessageStatus::Continue => {}, // TODO
                MessageStatus::Complete => {
                    trace!("Marking request tt={} as complete", req.thread_trace);
                    self.req_mut(req).complete = true;
                },
                MessageStatus::Timeout => {
                    self.sessions.get_mut(&session_id).unwrap().reset();
                    warn!("Stateful session ended by server on keepalive timeout");
                    return Err(error::Error::RequestTimeoutError);
                },
                _ => {
                    self.sessions.get_mut(&session_id).unwrap().reset();
                    warn!("Unexpected response status {}", stat.status());
                    return Err(error::Error::RequestTimeoutError);
                }
            }

            return Ok(None);
        }

        error!("handle_reply request tt={} unexpected response {}",
            req.thread_trace, msg.to_json_value().dump());

        self.sessions.get_mut(&session_id).unwrap().reset();

        return Err(error::Error::BadResponseError);
    }

    pub fn complete(&self, req: &ClientRequest) -> bool {
        let ses = self.ses(req.session_id);
        match ses.requests.get(&req.thread_trace) {
            Some(r) => r.complete && !ses.has_pending_replies(r.thread_trace),
            None => false,
        }
    }
}


