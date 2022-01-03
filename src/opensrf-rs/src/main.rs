use opensrf::message::MessageStatus;
use opensrf::message::MessageType;
use opensrf::message::TransportMessage;
use opensrf::message::Payload;
use opensrf::message::Method;
use opensrf::message::Message;
use opensrf::bus::Bus;
use opensrf::conf::BusConfig;

fn main() {

    let mut bs = BusConfig::new();
    bs.set_host("127.0.0.1");
    bs.set_port(6379);

    let mut bus = Bus::new(Bus::new_bus_id("client"));
    bus.connect(&bs).unwrap();

    let pl = Payload::Method(
        Method::new(
            "opensrf.system.echo",
            vec![json::from("Hello"), json::from("World")]
        )
    );

    let req = Message::new(MessageType::Request, 1, pl);
    let mut tm = TransportMessage::new("opensrf.settings", bus.bus_id(), "1234710324987");
    tm.body_as_mut().push(req);

    bus.send(&tm).unwrap();

    let resp = bus.recv(10).unwrap().unwrap();

    println!("RESP: {}", resp.to_json_value().dump());

}


