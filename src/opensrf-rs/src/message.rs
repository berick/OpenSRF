use std::fmt;

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

/*

impl fmt::Display for MessageType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", *self as isize)
    }
}

pub enum Payload {
    Request(Request),
    Response(Response),
    Error(String),
    NoPayload,
}

impl Payload {

    pub fn to_json_value(&self) -> json::JsonValue {
        match self {
            Payload::Request(pl) => pl.to_json_value(),
            Payload::Response(pl) => pl.to_json_value(),
            Payload::Error(pl) => json::from(pl.clone()),
            Payload::NoPayload => json::JsonValue::Null,
        }
    }
}

*/

pub struct Message {
    to: String,
    from: String,
    mtype: MessageType,
    req_id: u64,
    //payload: Payload,
}

/*

impl Message {

    pub fn new(to: &str, from: &str, req_id: u64, mtype: MessageType, payload: Payload) -> Self {
        Message {
            to: String::from(to),
            from: String::from(from),
            req_id,
            mtype,
            payload,
        }
    }

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

        let req_id = match json_obj["req_id"].as_u64() {
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
            // Propagate the req_id into the request
            req.set_req_id(req_id);
        };

        Some(Message {
            to: to.to_string(),
            from: from.to_string(),
            req_id: req_id,
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
            req_id: json::from(self.req_id),
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

    pub fn req_id(&self) -> u64 {
        self.req_id
    }

    pub fn mtype(&self) -> MessageType {
        self.mtype
    }

    pub fn payload(&self) -> &Payload {
        &self.payload
    }
}

/// Delivers a single API response.
///
/// Each Request will have zero or more associated Response messages.
#[derive(Clone)]
pub struct Response {
    /// API response value.
    value: json::JsonValue,
}

impl Response {

    pub fn new(value: json::JsonValue) -> Self {
        Response {
            value,
        }
    }

    pub fn value(&self) -> &json::JsonValue {
        &self.value
    }

    pub fn from_json_value(json_obj: &json::JsonValue) -> Option<Self> {

        // A response must have a 'value', even if it's a NULL.
        if !json_obj.has_key("value") { return None; }

        Some(Response::new(json_obj["value"].clone()))
    }

    pub fn to_json_value(&self) -> json::JsonValue {
        json::object!{
            value: self.value.clone(),
        }
    }
}

/// Delivers a single API request with method name and parameters.
pub struct Request {
    method: String,
    params: Vec<json::JsonValue>,
    /// If true, caller requests stateful communication.
    connect: bool,

    /// Request also needs a copy of its ID so the higher level
    /// API can access the value without maintaining a ref to the
    /// wrapper message.
    ///
    /// This is not packaged in the JSON.
    req_id: u64,
}

impl Request {

    pub fn new(req_id: u64, method: &str, params: Vec<json::JsonValue>, connect: bool) -> Self {
        Request {
            req_id,
            connect: connect,
            params: params,
            method: String::from(method),
        }
    }

    pub fn from_json_value(json_obj: &json::JsonValue) -> Option<Self> {

        let method = match json_obj["method"].as_str() {
            Some(m) => m.to_string(),
            None => { return None; }
        };

        let connect = match json_obj["connect"].as_bool() {
            Some(b) => b,
            None => { return None; }
        };

        let ref params = json_obj["params"];

        if !params.is_array() { return None; }

        let p = params.members().map(|p| p.clone()).collect();

        Some(Request {
            method: method,
            connect: connect,
            req_id: 0,
            params: p
        })
    }

    pub fn method(&self) -> &str {
        &self.method
    }

    pub fn params(&self) -> &Vec<json::JsonValue> {
        &self.params
    }

    pub fn connect(&self) -> bool {
        self.connect
    }

    pub fn req_id(&self) -> u64 {
        self.req_id
    }

    pub fn set_req_id(&mut self, req_id: u64) {
        self.req_id = req_id;
    }

    pub fn to_json_value(&self) -> json::JsonValue {

        // Clone the params so the new json object can absorb them.
        let params: Vec<json::JsonValue> =
            self.params.iter().map(|v| v.clone()).collect();

        json::object!{
            method: json::from(self.method()),
            params: json::from(params),
            connect: json::from(self.connect()),
        }
    }
}

*/


