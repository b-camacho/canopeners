use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering::SeqCst;
use canopeners::enums::EmergencyErrorRegister;
use canopeners::{
    Conn, Emergency, Guard, GuardStatus, Message, Nmt, Pdo, Sdo, SdoCmd, SdoCmdInitiateDownloadTx, Rxtx, SdoCmdDownloadSegmentTx
};


fn sender(done: &AtomicBool) {
    let mut conn = Conn::new("vcan0").unwrap();
    let pdo = Pdo::new(10, 1, &[3, 4, 0]);
    conn.send(&Message::Pdo(pdo)).unwrap();
    let nmt = Nmt::new(canopeners::NmtFunction::EnterOperational, 10);
    conn.send(&Message::Nmt(nmt)).unwrap();
    let emergency = Emergency::new(
        10,
        canopeners::enums::EmergencyErrorCode::AmbientTemperature,
        vec![EmergencyErrorRegister::Temperature],
        &[1, 2],
    );
    conn.send(&Message::Emergency(emergency)).unwrap();
    let guard = Guard::new(10, false, GuardStatus::Operational);
    conn.send(&Message::Guard(guard)).unwrap();


    conn.sdo_write(0x10, 0x1000, 1, &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10]).unwrap();
    done.store(true, std::sync::atomic::Ordering::SeqCst);
}

fn receiver(done: &AtomicBool) {
    let conn = Conn::new("vcan0").unwrap();
    conn.set_read_timeout(std::time::Duration::from_millis(10)).unwrap();
    while !done.load(SeqCst) {
        let msg = conn.recv();
        //dbg!(&msg);

        match msg {
            Ok(Message::Sdo(Sdo {
                command: SdoCmd::InitiateDownloadRx(payload),
                node_id,
                rxtx: Rxtx::RX,
            })) => {
                conn.send(&Message::Sdo(Sdo {
                    node_id,
                    rxtx: Rxtx::TX,
                    command: SdoCmd::InitiateDownloadTx(SdoCmdInitiateDownloadTx {
                        index: payload.index,
                        sub_index: payload.sub_index,
                    }),
                }))
            },
            Ok(Message::Sdo(Sdo {
                command: SdoCmd::DownloadSegmentRx(payload),
                node_id,
                rxtx: Rxtx::RX,
            })) => {
                conn.send(&Message::Sdo(Sdo {
                    node_id,
                    rxtx: Rxtx::TX,
                    command: SdoCmd::DownloadSegmentTx(SdoCmdDownloadSegmentTx {
                        toggle: payload.toggle,
                    }),
                }))
            },
            Err(canopeners::CanOpenError::IOError(_)) => { return },
            Ok(_) => Ok(()),
            Err(e) => Err(e),
        }.unwrap();
    }
}

fn main() {
    let done = std::sync::atomic::AtomicBool::new(false);
    std::thread::scope(|s| {
        s.spawn(|| { sender(&done) });
        s.spawn(|| { receiver(&done) });
    })
}
