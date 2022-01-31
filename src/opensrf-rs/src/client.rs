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

pub struct ClientRequest {
    pub request_id: usize,
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

    fn ses(&self, session_id: usize) -> &Session {
        self.sessions.get(&session_id).unwrap()
    }

    fn ses_mut(&mut self, session_id: usize) -> &mut Session {
        self.sessions.get_mut(&session_id).unwrap()
    }

    fn get_ses_by_req_mut(&mut self, request_id: usize) -> &mut Session {

        let mut session_id = 0;
        for mut ses in self.sessions.values() {
            if ses.requests.get(&request_id).is_some() {
                session_id = ses.session_id;
            }
        }

        self.ses_mut(session_id)
    }

    fn get_req_by_id(&self, request_id: usize) -> &Request {
        for ses in self.sessions.values() {
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
        let session_id = self.last_session_id;

        let ses = Session::new(service, session_id);
        self.sessions.insert(session_id, ses);

        session_id
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

    pub fn cleanup(&mut self, session_id: usize) {
        self.sessions.remove(&session_id);
    }

    pub fn connect(&mut self, session_id: usize) -> Result<(), error::Error> {

        let mut ses = self.sessions.get_mut(&session_id).unwrap();
        ses.last_thread_trace += 1;
        self.last_request_id += 1;

        let thread_trace = ses.last_thread_trace;
        let thread = ses.thread.to_string();
        let remote_addr = ses.remote_addr().to_string();

        ses.requests.insert(self.last_request_id,
            Request {
                complete: false,
                session_id: session_id,
                request_id: self.last_request_id,
                thread_trace: thread_trace,
            }
        );

        let req = ClientRequest {
            session_id: session_id,
            request_id: self.last_request_id,
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

    pub fn disconnect(&mut self, session_id: usize) -> Result<(), error::Error> {

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
        session_id: usize,
        method: &str,
        params: Vec<json::JsonValue>) -> Result<ClientRequest, error::Error> {

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

        self.last_request_id += 1;
        let mut ses = self.sessions.get_mut(&session_id).unwrap();

        trace!("request() session id={} adding request id={} tt={}",
            session_id, self.last_request_id, ses.last_thread_trace);

        ses.requests.insert(self.last_request_id,
            Request {
                complete: false,
                session_id: session_id,
                request_id: self.last_request_id,
                thread_trace: ses.last_thread_trace,
            }
        );

        Ok(ClientRequest {
            session_id: session_id,
            request_id: self.last_request_id,
            thread_trace: ses.last_thread_trace
        })
    }

    fn recv_from_backlog(&mut self, req: &ClientRequest) -> Option<message::Message> {
        let ses = self.ses_mut(req.session_id);

        trace!("recv_from_backlog() id={} tt={}", req.request_id, req.thread_trace);

        match ses.backlog.iter().position(|resp| resp.thread_trace() == req.thread_trace) {
            Some(index) => {
                trace!("recv_from_backlog() found response for request id={}", req.request_id);
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

        let request_id = req.request_id;
        let session_id = req.session_id;
        let thread_trace = req.thread_trace;

        trace!("recv() request_id={} thread_trace={}", request_id, thread_trace);

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

        let request_id = req.request_id;
        let session_id = req.session_id;

        if let Payload::Result(resp) = msg.payload() {
            trace!("handle_reply() found response for req={}", request_id);
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
                    trace!("Marking request {} as complete", request_id);
                    self.get_req_by_id_mut(request_id).complete = true;
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

        error!("handle_reply request={} unexpected response {}",
            request_id, msg.to_json_value().dump());

        self.sessions.get_mut(&session_id).unwrap().reset();

        return Err(error::Error::BadResponseError);
    }

    pub fn complete(&self, req: &ClientRequest) -> bool {
        let ses = self.ses(req.session_id);
        match ses.requests.get(&req.request_id) {
            Some(r) => r.complete && !ses.has_pending_replies(r.thread_trace),
            None => false,
        }
    }
}


