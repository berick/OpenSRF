use opensrf::bus::Bus;
use opensrf::conf::ClientConfig;
use opensrf::message::MessageStatus;
use opensrf::message::MessageType;
use opensrf::message::TransportMessage;
use opensrf::message::Payload;
use opensrf::message::Method;
use opensrf::message::Message;
use opensrf::client::Client;
use opensrf::client::Request;

fn main() {

    let mut conf = ClientConfig::new();
    conf.load_file("conf/opensrf_client.yml");

    let mut client = Client::new();
    client.bus_connect(conf.bus_config()).unwrap();

    let mut ses = client.session("opensrf.settings");

    let mut req = ses.request(
        "opensrf.system.echo",
        vec![json::from("Hello"), json::from("World")]
    ).unwrap();

    loop {
        match ses.recv(&mut req, 10).unwrap() {
            Some(value) => println!("GOT RESPONSE: {}", value.dump()),
            None => break,
        }
    }

    println!("Request is complete: {}", req.complete());
}


