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
}
/*

    /// Creates a Message from a JSON value.
    ///
    /// Returns None if the JSON value cannot be coerced into a Message.
    pub fn from_json_value(json_obj: &json::JsonValue) -> Option<Self> {

        let to = match json_obj["to"].as_str() {
            Some(s) => s,
            None => { return None; }
        };

        let from = match json_obj["from"].as_str() {
            Some(s) => s,
            None => { return None; }
        };

        let thread_trace = match json_obj["thread_trace"].as_u64() {
            Some(i) => i,
            None => { return None; }
        };

        let mtype_num = match json_obj["mtype"].as_isize() {
            Some(s) => s,
            None => { return None; }
        };

        let mtype = match MessageType::from_isize(mtype_num) {
            Some(mt) => mt,
            None => { return None; }
        };

        let mut payload = match
            Message::payload_from_json_value(mtype, &json_obj["payload"]) {
            Some(p) => p,
            None => { return None; }
        };

        if let Payload::Request(ref mut req) = payload {
            // Propagate the thread_trace into the request
            req.set_thread_trace(thread_trace);
        };

        Some(Message {
            to: to.to_string(),
            from: from.to_string(),
            thread_trace: thread_trace,
            mtype: mtype,
            payload: payload
        })
    }

    fn payload_from_json_value(mtype: MessageType,
        payload_obj: &json::JsonValue) -> Option<Payload> {

        match mtype {

            MessageType::Request => {
                match Request::from_json_value(payload_obj) {
                    Some(req) => Some(Payload::Request(req)),
                    _ => None
                }
            },

            MessageType::Response => {
                match Response::from_json_value(payload_obj) {
                    Some(resp) => Some(Payload::Response(resp)),
                    _ => None
                }
            },

            _ => {
                // Assumes any payload that is a bare string is an error message.
                if payload_obj.is_string() {
                    Some(Payload::Error(payload_obj.as_str().unwrap().to_string()))
                } else {
                    Some(Payload::NoPayload)
                }
            }
        }
    }

    pub fn to_json(&self) -> String {
        self.to_json_value().dump()
    }

    pub fn to_json_value(&self) -> json::JsonValue {

        let mut obj = json::object!{
            to: json::from(self.to()),
            from: json::from(self.from()),
            thread_trace: json::from(self.thread_trace),
            mtype: json::from(self.mtype as isize),
        };

        match self.payload {
            // Avoid adding the "payload" key for non-payload messages.
            Payload::NoPayload => {},
            _ => obj["payload"] = self.payload.to_json_value(),
        }

        obj
    }


    pub fn to(&self) -> &str {
        &self.to
    }

    pub fn from(&self) -> &str {
        &self.from
    }

    pub fn thread_trace(&self) -> u64 {
        self.thread_trace
    }

    pub fn mtype(&self) -> MessageType {
        self.mtype
    }

    pub fn payload(&self) -> &Payload {
        &self.payload
    }
}

*/

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



