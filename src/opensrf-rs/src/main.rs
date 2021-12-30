use opensrf::message::MessageStatus;
use opensrf::message::MessageType;

fn main() {
    println!("Hello, world!");

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
}
