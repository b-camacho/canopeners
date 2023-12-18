
fn sender() {
    let conn = canopeners::Conn::new("vcan0").unwrap();
    let msg = canopeners::Sdo{};
    conn.send(canopeners::Message::Sync(msg));

}

fn receiver() {
    let conn = canopeners::Conn::new("vcan0").unwrap();
    let msg = conn.recv();
    dbg!(msg);
}

fn main() {
    std::thread::scope(|s| {
        s.spawn(sender);
        s.spawn(receiver);
    })

}
