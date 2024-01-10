use canopeners::enums::EmergencyErrorRegister;
use canopeners::{Conn, Emergency, Guard, GuardStatus, Message, Nmt, Pdo, Sdo, SdoCmd};
fn sender() {
    let conn = Conn::new("vcan0").unwrap();
    let sdo = Sdo::new_write(10, 1, 2, Box::new([3, 4, 0]));
    conn.send(Message::Sdo(sdo)).unwrap();
    let pdo = Pdo::new(10, 1, &[3, 4, 0]);
    conn.send(Message::Pdo(pdo)).unwrap();
    let nmt = Nmt::new(canopeners::NmtFunction::EnterOperational, 10);
    conn.send(Message::Nmt(nmt)).unwrap();
    let emergency = Emergency::new(
        10,
        canopeners::enums::EmergencyErrorCode::AmbientTemperature,
        vec![EmergencyErrorRegister::Temperature],
        &[1, 2],
    );
    conn.send(Message::Emergency(emergency)).unwrap();
    let guard = Guard::new(10, false, GuardStatus::Operational);
    conn.send(Message::Guard(guard)).unwrap();
}

fn receiver() {
    let conn = Conn::new("vcan0").unwrap();
    loop {
        let msg = conn.recv();
        dbg!(msg.unwrap());
    }
}

fn main() {
    std::thread::scope(|s| {
        s.spawn(sender);
        s.spawn(receiver);
    })
}
