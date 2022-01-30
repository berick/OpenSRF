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

    /*
    let ses = client.session("opensrf.settings");

    ses.connect().unwrap();

    let params = vec![json::from("Hello"), json::from("World")];
    let rid = ses.request("opensrf.system.echo", params).unwrap();

    while !ses.request_complete(rid) {
        match ses.recv(rid, 10).unwrap() {
            Some(value) => println!("GOT RESPONSE: {}", value.dump()),
            None => break,
        }
    }

    ses.disconnect().unwrap();
    ses.cleanup();

    */

    let sid = client.session("opensrf.settings");

    client.connect(sid);

    let params = vec![json::from("Hello"), json::from("World")];

    let rid = client.request(sid, "opensrf.system.echo", params).unwrap();

    while !client.complete(rid) {
        match client.recv(rid, 10).unwrap() {
            Some(value) => println!("GOT RESPONSE: {}", value.dump()),
            None => break,
        }
    }

    client.disconnect(sid).unwrap();
    client.cleanup(sid);

    /*
    let mut ses = client.session("opensrf.settings");

    ses.connect().unwrap();

    let mut req = ses.request("opensrf.system.echo", params).unwrap();

    while !req.complete() {
        match req.recv(10).unwrap() {
            Some(value) => println!("GOT RESPONSE: {}", value.dump()),
            None => break,
        }
    }

    ses.disconnect().unwrap();
    ses.cleanup();

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


