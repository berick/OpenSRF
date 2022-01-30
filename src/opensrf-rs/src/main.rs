use opensrf::conf::ClientConfig;
use opensrf::message::MessageType;
use opensrf::message::MessageStatus;
use opensrf::message::TransportMessage;
use opensrf::message::Payload;
use opensrf::message::Method;
use opensrf::message::Message;
use opensrf::client::Client;

use redis;
use redis::Commands;
use std::{thread, time};

fn main() {

    let mut conf = ClientConfig::new();
    conf.load_file("conf/opensrf_client.yml");

    let mut client = Client::new();
    client.bus_connect(conf.bus_config()).unwrap();

    let ses = client.session("opensrf.settings");

    client.connect(ses).unwrap();

    let params = vec![json::from("Hello"), json::from("World")];
    let req = client.request(ses, "opensrf.system.echo", params).unwrap();

    while !client.complete(req) {
        match client.recv(req, 10).unwrap() {
            Some(value) => println!("GOT RESPONSE: {}", value.dump()),
            None => break,
        }
    }

    let params = vec![json::from("Hello"), json::from("World")];
    let req = client.request(ses, "opensrf.system.echo", params).unwrap();

    while !client.complete(req) {
        match client.recv(req, 10).unwrap() {
            Some(value) => println!("GOT RESPONSE: {}", value.dump()),
            None => break,
        }
    }

    client.disconnect(ses).unwrap();
    client.cleanup(ses);
}


