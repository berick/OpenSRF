use std::fmt;

// Derive is needed to do things like: let i = self.mtype as isize;
#[derive(Copy, Clone, PartialEq)]
pub enum MessageType {
    Request = 1, // value cascades
    Response,
    Continue,
    RequestComplete,
    Timeout,
    Disconnect,
    ServerError,
    MethodNotFound,
    Forbidden,
    BadRequest,
}

// Rust can convert an enum variant to an isize, but not the other way
// around, unless I just missed it.
impl MessageType {
    pub fn from_isize(i: isize) -> Option<Self> {
        match i {
            1  => Some(MessageType::Request),
            2  => Some(MessageType::Response),
            3  => Some(MessageType::Continue),
            4  => Some(MessageType::RequestComplete),
            5  => Some(MessageType::Timeout),
            6  => Some(MessageType::Disconnect),
            7  => Some(MessageType::ServerError),
            8  => Some(MessageType::MethodNotFound),
            9  => Some(MessageType::Forbidden),
            10 => Some(MessageType::BadRequest),
            _  => None
        }
    }
}

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

pub struct Message {
    to: String,
    from: String,
    mtype: MessageType,
    req_id: u64,
    payload: Payload,
}

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



