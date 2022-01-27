use opensrf::bus::Bus;
use opensrf::conf::ClientConfig;
use opensrf::message::MessageType;
use opensrf::message::MessageStatus;
use opensrf::message::TransportMessage;
use opensrf::message::Payload;
use opensrf::message::Method;
use opensrf::message::Message;
use opensrf::client::Client;

fn main() {

    let mut conf = ClientConfig::new();
    conf.load_file("conf/opensrf_client.yml");

    let mut client = Client::new();
    client.bus_connect(conf.bus_config()).unwrap();

    let thread = client.session("opensrf.settings");

    client.request(
        &thread,
        "opensrf.system.echo",
        vec![json::from("Hello"), json::from("World")]
    ).unwrap();

    while !client.request_is_complete(&thread).unwrap() {
        match client.recv(&thread, 10).unwrap() {
            Some(value) => println!("GOT RESPONSE: {}", value.dump()),
            None => break,
        }
    }

    println!("Request is complete? {}", client.request_is_complete(&thread).unwrap());

    client.request(
        &thread,
        "opensrf.system.echo",
        vec![json::from("Hello 2"), json::from("World 2")]
    ).unwrap();

    println!("Request 2 is complete? {}", client.request_is_complete(&thread).unwrap());

    while !client.request_is_complete(&thread).unwrap() {
        match client.recv(&thread, 10).unwrap() {
            Some(value) => println!("GOT RESPONSE: {}", value.dump()),
            None => break,
        }
    }


    /*
    client.connect(ses).unwrap();
    client.recv(ses)
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


