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

pub struct Client {
    bus: bus::Bus,

    sessions: HashMap<usize, Session>,

    /// Queue of receieved transport messages that have yet to be
    /// processed by any sessions.
    transport_backlog: Vec<message::TransportMessage>,

    last_session_id: usize,
    last_request_id: usize,
}

impl Client {

    pub fn new() -> Self {
        Client {
            bus: bus::Bus::new(bus::Bus::new_bus_id("client")),
            sessions: HashMap::new(),
            transport_backlog: Vec::new(),
            last_session_id: 0,
            last_request_id: 0,
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

    fn get_ses_by_req(&self, request_id: usize) -> &Session {
        for ses in self.sessions.values() {
            if ses.requests.get(&request_id).is_some() {
                return ses
            }
        }
        panic!("Request {} has no session", request_id);
    }

    fn get_ses_by_req_mut(&mut self, request_id: usize) -> &mut Session {

        let mut session_id = 0;
        for mut ses in self.sessions.values() {
            if ses.requests.get(&request_id).is_some() {
                session_id = ses.session_id;
            }
        }

        self.sessions.get_mut(&session_id).unwrap()
    }

    fn get_req_by_id(&self, request_id: usize) -> &Request {
        for ses in self.sessions.values() {
            trace!("Checking session {} {}", ses.session_id, ses.requests.len());
            let req_op = ses.requests.get(&request_id);
            if req_op.is_some() { return req_op.unwrap(); }
        }
        panic!("No Request with id {}", request_id);
    }

    fn get_req_by_id_mut(&mut self, request_id: usize) -> &mut Request {
        for ses in self.sessions.values_mut() {
            let req_op = ses.requests.get_mut(&request_id);
            if req_op.is_some() { return req_op.unwrap(); }
        }
        panic!("No Request with id {}", request_id);
    }


    pub fn session(&mut self, service: &str) -> usize {
        self.last_session_id += 1;
        let ses_id = self.last_session_id;

        let ses = Session::new(service, ses_id);
        self.sessions.insert(ses_id, ses);

        ses_id
    }

    /// Returns the first transport message pulled from the pending
    /// messages queue that matches the provided thread.
    fn recv_session_from_backlog(&mut self,
        thread: &str, mut timeout: i32) -> Option<TransportMessage> {

        if let Some(index) =
            self.transport_backlog.iter().position(|tm| tm.thread() == thread) {

            trace!("Found a stashed reply for thread {}", thread);

            let tm = self.transport_backlog.remove(index);

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

        let thread = match self.sessions.get(&session_id) {
            Some(ses) => ses.thread.to_string(),
            None => { return Err(error::Error::NoSuchThreadError); },
        };

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

        match self.sessions.get_mut(&session_id) {
            Some(ses) => {
                if ses.remote_addr() != tm.from() {
                    ses.remote_addr = Some(tm.from().to_string());
                }
            },
            None => {
                return Err(error::Error::NoSuchThreadError);
            }
        };

        Ok(Some(tm))
    }

    pub fn cleanup(&mut self, session_id: usize) {
        self.sessions.remove(&session_id);
    }

    pub fn connect(&mut self, session_id: usize) -> Result<(), error::Error> {

        let mut ses = self.sessions.get_mut(&session_id).unwrap();
        let thread_trace = ses.last_thread_trace;
        let thread = ses.thread.to_string();
        let remote_addr = ses.remote_addr().to_string();

        ses.last_thread_trace += 1;

        ses.requests.insert(self.last_request_id,
            Request {
                complete: false,
                session_id: session_id,
                request_id: self.last_request_id,
                thread_trace: thread_trace,
            }
        );

        let msg = Message::new(MessageType::Connect,
            thread_trace, message::Payload::NoPayload);

        let tm = TransportMessage::new_with_body(
            &remote_addr, self.bus.bus_id(), &thread, msg);

        self.bus.send(&tm)?;

        let mut timeout = CONNECT_TIMEOUT;

        while timeout > 0 {

            let start = time::SystemTime::now();

            trace!("connect() calling receive with timeout={}", timeout);
            let recv_op = self.recv(thread_trace, timeout)?;

            if let Some(ses) = self.sessions.get_mut(&session_id) {
                if ses.connected {
                    return Ok(())
                }
            }

            timeout -= start.elapsed().unwrap().as_secs() as i32;
        }

        Err(error::Error::ConnectTimeoutError)
    }

    pub fn disconnect(&mut self, session_id: usize) -> Result<(), error::Error> {

        let mut thread: String;
        let mut remote_addr: String;
        let mut thread_trace = 0;

        match self.sessions.get(&session_id) {
            Some(ses) => {
                thread_trace = ses.last_thread_trace;
                thread = ses.thread.to_string();
                remote_addr = ses.remote_addr().to_string();
            },
            None => {
                return Err(error::Error::NoSuchThreadError);
            }
        }

        let msg = Message::new(MessageType::Disconnect,
            thread_trace, message::Payload::NoPayload);

        let tm = TransportMessage::new_with_body(
            &remote_addr, self.bus.bus_id(), &thread, msg);

        self.bus.send(&tm)?;

        // Avoid changing remote_addr until above message is composed.
        if let Some(ses) = self.sessions.get_mut(&session_id) {
            ses.reset();
        }

        Ok(())
    }

    pub fn request(
        &mut self,
        session_id: usize,
        method: &str,
        params: Vec<json::JsonValue>) -> Result<usize, error::Error> {

        let mut ses = self.sessions.get_mut(&session_id).unwrap();
        let thread_trace = ses.last_thread_trace;
        let thread = ses.thread.to_string();
        let remote_addr = ses.remote_addr().to_string();

        ses.last_thread_trace += 1;

        let method = Payload::Method(Method::new(method, params));

        let req = Message::new(MessageType::Request, thread_trace, method);

        let tm = TransportMessage::new_with_body(
            &remote_addr, self.bus.bus_id(), &thread, req);

        self.bus.send(&tm)?;

        self.last_request_id += 1;
        let mut ses = self.sessions.get_mut(&session_id).unwrap();

        trace!("session {} adding request {}", session_id, self.last_request_id);

        ses.requests.insert(self.last_request_id,
            Request {
                complete: false,
                session_id: session_id,
                request_id: self.last_request_id,
                thread_trace: thread_trace,
            }
        );

        Ok(self.last_request_id)
    }

    fn recv_from_backlog(&mut self, request_id: usize) -> Option<message::Message> {

        let ses = self.get_ses_by_req_mut(request_id);

        let req = match ses.requests.get(&request_id) {
            Some(r) => r,
            None => { return None; }
        };

        match ses.backlog.iter().position(|resp| resp.thread_trace() == req.thread_trace) {
            Some(index) => Some(ses.backlog.remove(index)),
            None => None,
        }
    }

    pub fn recv(
        &mut self,
        request_id: usize,
        timeout: i32) -> Result<Option<json::JsonValue>, error::Error> {

        let mut resp: Result<Option<json::JsonValue>, error::Error> = Ok(None);
        let mut req = self.get_req_by_id(request_id);

        let session_id = req.session_id;
        let thread_trace = req.thread_trace;

        // Reply for this request was previously pulled from the
        // bus and tossed into our queue.
        if let Some(msg) = self.recv_from_backlog(request_id) {
            return self.handle_reply(session_id, request_id, &msg);
        }

        let tm = match self.recv_session(session_id, timeout)? {
            Some(m) => m,
            None => { return resp; }
        };

        let mut msg_list = tm.body().to_owned();

        if msg_list.len() == 0 { return Ok(None); }

        let msg = msg_list.remove(0);

        if msg.thread_trace() == thread_trace {

            // We have received a reply to the provided request.
            resp = self.handle_reply(session_id, request_id, &msg);

        } else {

            /// Pulled a reply to a request made by this client session
            /// but not matching the requested thread trace.

            trace!("recv() found a reply for a request {}, stashing",
                msg.thread_trace());

            if let Some(ses) = self.sessions.get_mut(&session_id) {
                ses.backlog.push(msg);
            }
        }

        while msg_list.len() > 0 {
            if let Some(ses) = self.sessions.get_mut(&session_id) {
                ses.backlog.push(msg_list.remove(0));
            }
        }

        return resp;
    }

    fn handle_reply(
        &mut self,
        session_id: usize,
        request_id: usize,
        msg: &message::Message) -> Result<Option<json::JsonValue>, error::Error> {

        if let Payload::Result(resp) = msg.payload() {
            return Ok(Some(resp.content().clone()));
        };

        let ses = self.sessions.get_mut(&session_id).unwrap();

        if let Payload::Status(stat) = msg.payload() {

            match stat.status() {
                MessageStatus::Ok => ses.connected = true,
                MessageStatus::Continue => {}, // TODO
                MessageStatus::Complete => {
                    self.get_req_by_id_mut(request_id).complete = true;
                    self.sessions.get_mut(&session_id).unwrap().reset();
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

        error!("Request::recv() unexpected response {}", msg.to_json_value().dump());

        self.sessions.get_mut(&session_id).unwrap().reset();

        return Err(error::Error::BadResponseError);
    }

    pub fn complete(&self, req_id: usize) -> bool {
        let ses = self.get_ses_by_req(req_id);
        match ses.requests.get(&req_id) {
            Some(req) => req.complete,
            None => false,
        }
    }
}


