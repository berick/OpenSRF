use opensrf::message::MessageStatus;
use opensrf::message::MessageType;
use opensrf::message::TransportMessage;
use opensrf::message::Payload;
use opensrf::message::Method;
use opensrf::message::Message;

fn main() {

    let pl = Payload::Method(
        Method::new(
            "opensrf.system.echo",
            vec![json::from("Hello"), json::from("World")]
        )
    );

    let req = Message::new(MessageType::Request, 1, pl);
    let mut tm = TransportMessage::new("my-to", "my-from", "my-thread");
    tm.body_as_mut().push(req);

    println!("MESSAGE: {}", tm.to_json_value().dump());

    let json_str = r#"{"to":"my-to","from":"my-from","thread":"my-thread","body":[{"__c":"osrfMessage","__p":{"threadTrace":1,"type":"REQUEST","locale":"en-US","timezone":"America/New_York","api_level":1,"ingress":"opensrf","payload":{"__c":"osrfMethod","__p":{"method":"opensrf.system.echo","params":["Hello","World"]}}}}]}"#;

    let json_value = json::parse(json_str).unwrap();
    let tm = TransportMessage::from_json_value(&json_value).unwrap();

    if let Payload::Method(method) = tm.body()[0].payload() {
        println!("param 1 {}", method.params()[0].as_str().unwrap());
    }

    println!("\n\nMESSAGE AGAIN: {}", tm.to_json_value().dump());
}


