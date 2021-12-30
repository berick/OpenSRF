use std::fmt;

const DEFAULT_LOCALE: &str = "en-US";
const DEFAULT_TIMEZONE: &str = "America/New_York";
const DEFAULT_API_LEVEL: u8 = 1;
const DEFAULT_INGRESS: &str = "opensrf";

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum MessageType {
    Connect,
    Request,
    Result,
    Status,
    Disconnect,
    Unknown,
}

impl From<&str> for MessageType {
    fn from(s: &str) -> Self {
        match s {
            "CONNECT"    => MessageType::Connect,
            "REQUEST"    => MessageType::Request,
            "RESULT"     => MessageType::Result,
            "STATUS"     => MessageType::Status,
            "DISCONNECT" => MessageType::Disconnect,
            _            => MessageType::Unknown,
        }
    }
}

impl Into<&'static str> for MessageType {
	fn into(self) -> &'static str {
        match self {
            MessageType::Connect    => "CONNECT",
            MessageType::Request    => "REQUEST",
            MessageType::Result     => "RESULT",
            MessageType::Status     => "STATUS",
            MessageType::Disconnect => "DISCONNECT",
            _                       => "UNKNOWN"
        }
    }
}

impl fmt::Display for MessageType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let s: &str = (*self).into();
        write!(f, "{}", s)
    }
}

// Derive is needed to do things like: let i = self.mtype as isize;
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum MessageStatus {
    Continue            = 100,
    Ok                  = 200,
    Accepted            = 202,
    Nocontent           = 204,
    Complete            = 205,
    Partial             = 206,
    Redirected          = 307,
    BadRequest          = 400,
    Unauthorized        = 401,
    Forbidden           = 403,
    NotFound            = 404,
    NotAllowed          = 405,
    Timeout             = 408,
    Expfailed           = 417,
    InternalServErerror = 500,
    NotImplemented      = 501,
    ServiceUnavailable  = 503,
    VersionNotSupported = 505,
    Unknown,
}

impl From<isize> for MessageStatus {
    fn from(num: isize) -> Self {
        match num {
            100 => MessageStatus::Continue,
            200 => MessageStatus::Ok,
            202 => MessageStatus::Accepted,
            204 => MessageStatus::Nocontent,
            205 => MessageStatus::Complete,
            206 => MessageStatus::Partial,
            307 => MessageStatus::Redirected,
            400 => MessageStatus::BadRequest,
            401 => MessageStatus::Unauthorized,
            403 => MessageStatus::Forbidden,
            404 => MessageStatus::NotFound,
            405 => MessageStatus::NotAllowed,
            408 => MessageStatus::Timeout,
            417 => MessageStatus::Expfailed,
            500 => MessageStatus::InternalServErerror,
            501 => MessageStatus::NotImplemented,
            503 => MessageStatus::ServiceUnavailable,
            505 => MessageStatus::VersionNotSupported,
            _   => MessageStatus::Unknown,
        }
    }
}

impl Into<&'static str> for MessageStatus {
	fn into(self) -> &'static str {
        match self {
            MessageStatus::Ok                   => "OK",
            MessageStatus::Continue             => "Continue",
            MessageStatus::Complete             => "Request Complete",
            MessageStatus::BadRequest           => "Bad Request",
            MessageStatus::Timeout              => "Timeout",
            MessageStatus::NotFound             => "Not Found",
            MessageStatus::NotAllowed           => "Not Allowed",
            MessageStatus::InternalServErerror  => "Internal Server Error",
            _                                   => "See Status Code",
        }
    }
}

impl fmt::Display for MessageStatus {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "({}) {:?}", *self as isize, self)
    }
}

pub enum Payload {
    Method(Method),
    Result(Result),
    Status(Status),
    NoPayload,
}

impl Payload {
    pub fn to_json_value(&self) -> json::JsonValue {
        match self {
            Payload::Method(pl) => pl.to_json_value(),
            Payload::Result(pl) => pl.to_json_value(),
            Payload::Status(pl) => pl.to_json_value(),
            Payload::NoPayload => json::JsonValue::Null,
        }
    }
}

pub struct TransportMessage {
    to: String,
    from: String,
    thread: String,
    osrf_xid: String,
    body: Vec<Message>,
}

impl TransportMessage {
    pub fn new(to: &str, from: &str, thread: &str) -> Self {
        TransportMessage {
            to: to.to_string(),
            from: from.to_string(),
            thread: thread.to_string(),
            osrf_xid: String::from(""),
            body: Vec::new()
        }
    }

    pub fn to(&self) -> &str {
        &self.to
    }

    pub fn from(&self) -> &str {
        &self.from
    }

    pub fn thread(&self) -> &str {
        &self.thread
    }

    pub fn body(&self) -> &Vec<Message> {
        &self.body
    }

    pub fn body_as_mut(&mut self) -> &mut Vec<Message> {
        &mut self.body
    }

    pub fn osrf_xid(&self) -> &str {
        &self.osrf_xid
    }

    pub fn set_osrf_xid(&mut self, xid: &str) {
        self.osrf_xid = xid.to_string()
    }

    pub fn from_json_value(json_obj: &json::JsonValue) -> Option<Self> {

        let to = match json_obj["to"].as_str() {
            Some(i) => i,
            None => { return None; }
        };

        let from = match json_obj["from"].as_str() {
            Some(i) => i,
            None => { return None; }
        };

        let thread = match json_obj["thread"].as_str() {
            Some(i) => i,
            None => { return None; }
        };

        let mut tmsg = TransportMessage::new(&to, &from, &thread);

        if let Some(xid) = json_obj["osrf_xid"].as_str() {
            tmsg.set_osrf_xid(xid);
        };

        if let json::JsonValue::Array(ref arr) = json_obj["body"] {
            for body in arr {
                if let Some(b) = Message::from_json_value(&body) {
                    tmsg.body_as_mut().push(b);
                }
            }
        }

        Some(tmsg)
    }

    pub fn to_json_value(&self) -> json::JsonValue {

        let mut body_arr = json::JsonValue::new_array();

        for body in self.body() {
            body_arr.push(body.to_json_value());
        }

        let mut obj = json::object!{
            to: json::from(self.to.clone()),
            from: json::from(self.from.clone()),
            thread: json::from(self.thread.clone()),
            body: body_arr,
        };

        obj
    }
}

pub struct Message {
    mtype: MessageType,
    thread_trace: u64,
    locale: String,
    timezone: String,
    api_level: u8,
    ingress: String,
    payload: Payload,
}

impl Message {

    pub fn new(mtype: MessageType, thread_trace: u64, payload: Payload) -> Self {
        Message {
            mtype,
            thread_trace,
            payload,
            api_level: DEFAULT_API_LEVEL,
            locale: DEFAULT_LOCALE.to_string(),
            timezone: DEFAULT_TIMEZONE.to_string(),
            ingress: DEFAULT_INGRESS.to_string(),
        }
    }

    pub fn mtype(&self) -> &MessageType {
        &self.mtype
    }

    pub fn thread_trace(&self) -> u64 {
        self.thread_trace
    }

    pub fn payload(&self) -> &Payload {
        &self.payload
    }

    pub fn api_level(&self) -> u8 {
        self.api_level
    }

    pub fn set_api_level(&mut self, level: u8) {
        self.api_level = level;
    }

    pub fn locale(&self) -> &str {
        &self.locale
    }

    pub fn set_locale(&mut self, locale: &str) {
        self.locale = locale.to_string()
    }

    pub fn timezone(&self) -> &str {
        &self.timezone
    }

    pub fn set_timezone(&mut self, timezone: &str) {
        self.timezone = timezone.to_string()
    }

    pub fn ingress(&self) -> &str {
        &self.ingress
    }

    pub fn set_ingress(&mut self, ingress: &str) {
        self.ingress = ingress.to_string()
    }

    /// Creates a Message from a JSON value.
    ///
    /// Returns None if the JSON value cannot be coerced into a Message.
    pub fn from_json_value(json_obj: &json::JsonValue) -> Option<Self> {

        let thread_trace = match json_obj["thread_trace"].as_u64() {
            Some(i) => i,
            None => { return None; }
        };

        let mtype_str = match json_obj["type"].as_str() {
            Some(s) => s,
            None => { return None; }
        };

        let mtype: MessageType = mtype_str.into();

        let mut payload = match
            Message::payload_from_json_value(mtype, &json_obj["payload"]) {
            Some(p) => p,
            None => { return None; }
        };

        let mut msg = Message::new(mtype, thread_trace, payload);

        // TODO set locale, etc.

        Some(msg)
    }

    fn payload_from_json_value(mtype: MessageType,
        payload_obj: &json::JsonValue) -> Option<Payload> {

        match mtype {

            MessageType::Request => {
                match Method::from_json_value(payload_obj) {
                    Some(method) => Some(Payload::Method(method)),
                    _ => None
                }
            },

            MessageType::Result => {
                match Result::from_json_value(payload_obj) {
                    Some(res) => Some(Payload::Result(res)),
                    _ => None
                }
            },

            MessageType::Status => {
                match Status::from_json_value(payload_obj) {
                    Some(stat) => Some(Payload::Status(stat)),
                    _ => None
                }
            },

            _ => Some(Payload::NoPayload),
        }
    }

    pub fn to_json_value(&self) -> json::JsonValue {

        let mut obj = json::object!{
            threadTrace: json::from(self.thread_trace),
            type: json::from(self.mtype as isize),
            locale: json::from(self.locale.clone()),
            timezone: json::from(self.timezone.clone()),
            api_level: json::from(self.api_level),
            ingress: json::from(self.ingress.clone()),
        };

        match self.payload {
            // Avoid adding the "payload" key for non-payload messages.
            Payload::NoPayload => {},
            _ => obj["payload"] = self.payload.to_json_value(),
        }

        obj
    }
}

/// Delivers a single API response.
///
/// Each Request will have zero or more associated Response messages.
#[derive(Debug, Clone)]
pub struct Result {
    status: MessageStatus,

    status_label: String,

    /// API response value.
    content: json::JsonValue,
}

impl Result {

    pub fn new(status: MessageStatus, status_label: &str, content: json::JsonValue) -> Self {
        Result {
            status,
            content,
            status_label: status_label.to_string(),
        }
    }

    pub fn content(&self) -> &json::JsonValue {
        &self.content
    }

    pub fn status(&self) -> &MessageStatus {
        &self.status
    }

    pub fn status_label(&self) -> &str {
        &self.status_label
    }

    pub fn from_json_value(json_obj: &json::JsonValue) -> Option<Self> {

        let code = match json_obj["statusCode"].as_isize() {
            Some(c) => c,
            None => { return None; },
        };

        let stat: MessageStatus = code.into();

        // If the message contains a status label, use it, otherwise
        // use the label associated locally with the status code
        let stat_str: &str = match json_obj["status"].as_str() {
            Some(s) => &s,
            None => stat.into(),
        };

        Some(Result::new(stat, stat_str, json_obj["content"].clone()))
    }

    pub fn to_json_value(&self) -> json::JsonValue {
        json::object!{
            status: json::from(self.status_label.clone()),
            statusCode: json::from(self.status as isize),
            content: self.content.clone(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Status {
    status: MessageStatus,
    status_label: String,
}

impl Status {

    pub fn new(status: MessageStatus, status_label: &str) -> Self {
        Status {
            status,
            status_label: status_label.to_string(),
        }
    }

    pub fn status(&self) -> &MessageStatus {
        &self.status
    }

    pub fn status_label(&self) -> &str {
        &self.status_label
    }

    pub fn from_json_value(json_obj: &json::JsonValue) -> Option<Self> {

        let code = match json_obj["statusCode"].as_isize() {
            Some(c) => c,
            None => { return None; },
        };

        let stat: MessageStatus = code.into();

        // If the message contains a status label, use it, otherwise
        // use the label associated locally with the status code
        let stat_str: &str = match json_obj["status"].as_str() {
            Some(s) => &s,
            None => stat.into(),
        };

        Some(Status::new(stat, stat_str))
    }

    pub fn to_json_value(&self) -> json::JsonValue {
        json::object!{
            status: json::from(self.status_label.clone()),
            statusCode: json::from(self.status as isize),
        }
    }
}


/// A single API request with method name and parameters.
pub struct Method {
    method: String,
    params: Vec<json::JsonValue>,
}

impl Method {

    pub fn new(method: &str, params: Vec<json::JsonValue>) -> Self {
        Method {
            params: params,
            method: String::from(method),
        }
    }

    pub fn from_json_value(json_obj: &json::JsonValue) -> Option<Self> {

        let method = match json_obj["method"].as_str() {
            Some(m) => m.to_string(),
            None => { return None; }
        };

        let ref params = json_obj["params"];

        if !params.is_array() { return None; }

        let p = params.members().map(|p| p.clone()).collect();

        Some(Method {
            method: method,
            params: p
        })
    }

    pub fn method(&self) -> &str {
        &self.method
    }

    pub fn params(&self) -> &Vec<json::JsonValue> {
        &self.params
    }

    pub fn to_json_value(&self) -> json::JsonValue {

        // Clone the params so the new json object can absorb them.
        let params: Vec<json::JsonValue> =
            self.params.iter().map(|v| v.clone()).collect();

        json::object!{
            method: json::from(self.method()),
            params: json::from(params),
        }
    }
}



