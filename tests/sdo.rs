use canopeners::{
    enums::EmergencyErrorRegister, Conn, Emergency, Guard, GuardStatus, Message, Nmt, Pdo, Rxtx,
    Sdo, SdoCmd, SdoCmdDownloadSegmentTx, SdoCmdInitiateDownloadTx, SdoCmdInitiatePayload,
    SdoCmdInitiateUploadRx, SdoCmdInitiateUploadTx, SdoCmdUploadSegmentRx, SdoCmdUploadSegmentTx,
};
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering::SeqCst;

fn sender(done: &AtomicBool) {
    let mut conn = Conn::new("vcan0").unwrap();
    let pdo = Pdo::new(10, 1, &[3, 4, 0]).unwrap();
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

    conn.sdo_write(0x10, 0x1000, 1, &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10])
        .unwrap();

    let sdo_read_res = conn.sdo_read(0x10, 0x1000, 1).unwrap();
    let sdo_read_exp: Box<[u8]> = Box::new([10, 9, 8, 7, 6, 5, 4, 3, 2, 1]);
    assert_eq!(sdo_read_exp, sdo_read_res);

    done.store(true, std::sync::atomic::Ordering::SeqCst);
}

fn receiver(done: &AtomicBool) {
    let conn = Conn::new("vcan0").unwrap();
    conn.set_read_timeout(std::time::Duration::from_millis(10))
        .unwrap();
    conn.set_write_timeout(std::time::Duration::from_millis(10))
        .unwrap();
    let mut download_data = Vec::new();
    let upload_data = &[10, 9, 8, 7, 6, 5, 4, 3, 2, 1];
    let mut upload_data_iter = 0;
    while !done.load(SeqCst) {
        let msg = conn.recv();

        match msg {
            Ok(Message::Sdo(Sdo {
                command: SdoCmd::InitiateDownloadRx(payload),
                node_id,
                rxtx: Rxtx::RX,
            })) => conn.send(&Message::Sdo(Sdo {
                node_id,
                rxtx: Rxtx::TX,
                command: SdoCmd::InitiateDownloadTx(SdoCmdInitiateDownloadTx {
                    index: payload.index,
                    sub_index: payload.sub_index,
                }),
            })),

            Ok(Message::Sdo(Sdo {
                command: SdoCmd::DownloadSegmentRx(payload),
                node_id,
                rxtx: Rxtx::RX,
            })) => {
                download_data.extend_from_slice(&payload.data);
                conn.send(&Message::Sdo(Sdo {
                    node_id,
                    rxtx: Rxtx::TX,
                    command: SdoCmd::DownloadSegmentTx(SdoCmdDownloadSegmentTx {
                        toggle: payload.toggle,
                    }),
                }))
            }

            Ok(Message::Sdo(Sdo {
                node_id,
                rxtx: Rxtx::RX,
                command: SdoCmd::InitiateUploadRx(SdoCmdInitiateUploadRx { index, sub_index }),
            })) => conn.send(&Message::Sdo(Sdo {
                node_id,
                rxtx: Rxtx::TX,
                command: SdoCmd::InitiateUploadTx(SdoCmdInitiateUploadTx {
                    index,
                    sub_index,
                    payload: SdoCmdInitiatePayload::Segmented(Some(10)),
                }),
            })),

            Ok(Message::Sdo(Sdo {
                node_id,
                rxtx: Rxtx::RX,
                command: SdoCmd::UploadSegmentRx(SdoCmdUploadSegmentRx { toggle }),
            })) => {
                let last = upload_data_iter + 7 > upload_data.len();
                let data = &upload_data
                    [upload_data_iter..(std::cmp::min(upload_data_iter + 7, upload_data.len()))];
                upload_data_iter += 7;
                conn.send(&Message::Sdo(Sdo {
                    node_id,
                    rxtx: Rxtx::TX,
                    command: SdoCmd::UploadSegmentTx(SdoCmdUploadSegmentTx {
                        toggle,
                        data: data.into(),
                        last,
                    }),
                }))
            }

            Err(canopeners::CanOpenError::IOError(_)) => return,
            Ok(_) => Ok(()),
            Err(e) => Err(e),
        }
        .unwrap();
    }

    assert_eq!(&[1, 2, 3, 4, 5, 6, 7, 8, 9, 10], download_data.as_slice());
}

#[test]
fn send_recv() {
    let done = std::sync::atomic::AtomicBool::new(false);
    std::thread::scope(|s| {
        s.spawn(|| sender(&done));
        s.spawn(|| receiver(&done));
    })
}
