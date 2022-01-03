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

    /*
    let s = MessageStatus::Ok;

    let ss: MessageStatus = 200.into();

    println!("s = {:?}", ss);

    println!("s = {}", ss);

    let t = MessageType::Connect;

    //println!("s = {}", s as isize);

    let mtype: MessageType = "CONNECT".into();
    println!("t = {}", mtype);

    let mtype_str: &str = mtype.into();

    println!("t = {}", mtype_str);
    */
}
