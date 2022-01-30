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

    Client::init_singleton();
    let mut client = Client::singleton();

    Client::bus_connect(conf.bus_config()).unwrap();

    let mut ses = client.session("opensrf.settings");

    /*

    ses.connect().unwrap();

    let params = vec![json::from("Hello"), json::from("World")];

    let mut req = ses.request("opensrf.system.echo", params).unwrap();

    while !req.complete() {
        match req.recv(10).unwrap() {
            Some(value) => println!("GOT RESPONSE: {}", value.dump()),
            None => break,
        }
    }

    ses.disconnect().unwrap();
    ses.cleanup();
    */

    /*

    ses.connect().unwrap();

    ses.request(
        "opensrf.system.echo",
        vec![json::from("Hello"), json::from("World")]
    ).unwrap();

    while !ses.request_complete() {
        match ses.recv(10).unwrap() {
            Some(value) => println!("GOT RESPONSE: {}", value.dump()),
            None => break,
        }
    }

    ses.request(
        "opensrf.system.echo",
        vec![json::from("Hello"), json::from("World")]
    ).unwrap();

    while !ses.request_complete() {
        match ses.recv(10).unwrap() {
            Some(value) => println!("GOT RESPONSE: {}", value.dump()),
            None => break,
        }
    }

    ses.disconnect().unwrap();
    */
}


