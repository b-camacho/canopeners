use socketcan::Id;
use socketcan::Frame;
use socketcan::EmbeddedFrame;
use binrw::{
    binrw,
    BinRead,
    BinWrite,
};

pub mod enums;



trait FrameRW {
    fn encode(&self, frame: &mut socketcan::CanFrame);
    fn decode(frame: &socketcan::CanFrame) -> Result<Self, CanOpenError> where Self : Sized;
    //fn id(&self) -> Option<u16>;

}

// CAN ids can only be standard (11bit), not extended (29bit)
// but CANOpen only uses standard
// returns None when CanFrame has extended ID
fn id_as_raw_std(frame: &socketcan::CanFrame) -> Result<u16, CanOpenError> {
    if let Id::Standard(sid) = frame.id() {
        Ok(sid.as_raw())
    } else {
        Err(CanOpenError::CanVersion("got extended (29bit) id, expected standard (11bit) id".to_owned()))
    }
}


// todo: I think node_ids are u8s actually
fn u16_as_id(id: u16) -> socketcan::StandardId {
    socketcan::StandardId::new(id).unwrap()
}

fn mk_can_frame(cob_id: u16, data: &[u8]) -> socketcan::CanDataFrame {
    socketcan::CanDataFrame::new(socketcan::StandardId::new(cob_id).unwrap(), data).unwrap()
}

#[binrw]
#[brw(little)]
#[derive(Clone, Debug)]
pub struct Nmt {
    pub function: NmtFunction,
    pub target_node: u8,
}

impl Nmt {
    pub fn new(function: NmtFunction, target_node: u8) -> Self {
        Self { function, target_node }
    }
}

impl FrameRW for Nmt {
    fn decode(frame: &socketcan::CanFrame) -> Result<Nmt, CanOpenError> {
        let mut c = std::io::Cursor::new(frame.data());
        Nmt::read(&mut c).map_err(|binrw_err| CanOpenError::ParseError(binrw_err.to_string()))

    }

    fn encode(&self, frame: &mut socketcan::CanFrame) {
        frame.set_id(u16_as_id(0x000));
        let mut c = std::io::Cursor::new(Vec::new());
        self.write(&mut c);
        frame.set_data(c.get_ref());
    }

}

#[binrw]
#[br(repr(u8))]
#[bw(repr(u8))]
#[derive(Clone, Debug)]
pub enum NmtFunction {
    EnterOperational = 0x01,
    EnterStop = 0x02,
    EnterPreOperational = 0x80,
    ResetNode = 0x81,
    ResetCommunication = 0x82,
}


#[binrw]
#[brw(little)]
#[derive(Debug)]
pub struct Emergency {
    #[brw(ignore)]
    node_id: u8,

    #[br(temp)]
    #[bw(calc =  enums::EmergencyErrorCode::encode(error_code))]
    error_code_raw: u16,

    #[br(try_calc = enums::EmergencyErrorCode::decode(error_code_raw))]
    #[bw(ignore)]
    error_code: enums::EmergencyErrorCode,

    #[br(temp)]
    #[bw(calc = enums::EmergencyErrorRegister::encode(error_register))]
    error_register_raw: u8,

    #[br(calc = enums::EmergencyErrorRegister::decode(error_register_raw))]
    #[bw(ignore)]
    error_register: Vec<enums::EmergencyErrorRegister>,

    vendor_specific: [u8; 5],

}

impl Emergency {
    pub fn new(node_id: u8,
               error_code: enums::EmergencyErrorCode,
               error_register: Vec<enums::EmergencyErrorRegister>,
               vendor_specific: &[u8]) -> Self {
        Self {
            node_id,
            error_code,
            error_register,
            vendor_specific: Self::to_vendor_specific(vendor_specific), 

        }
    }

    fn to_vendor_specific(data: &[u8]) -> [u8; 5] {
        let mut arr = [0u8; 5];
        arr[0..data.len()].copy_from_slice(data);
        arr
    }

}


impl FrameRW for Emergency {
    fn decode(frame: &socketcan::CanFrame) -> Result<Emergency, CanOpenError> {
        let data = frame.data();
        if data.len() < 8 {
            return Err(CanOpenError::ParseError("not a valid Emergency message, need at least 8 bytes".to_owned()));
        }
        let id = id_as_raw_std(frame)?;
        Emergency::read(&mut std::io::Cursor::new(data))
            .map_err(|e| CanOpenError::ParseError(format!("binrw err: {e}")))
            .map(|mut m| { m.node_id = (id - 0x080) as u8; m})
    }

    fn encode(&self, frame: &mut socketcan::CanFrame) {
        frame.set_id(u16_as_id(0x80 + self.node_id as u16));
        let mut c = std::io::Cursor::new(Vec::new());
        self.write(&mut c);
        frame.set_data(c.get_ref());
    }
}



#[derive(Debug)]
pub struct Sdo {
    pub node_id: u8, // Derived from the header
    pub command: SdoCmd, // command specifier
    pub rxtx: Rxtx,
}

#[derive(Debug)]
pub enum SdoCmd {
    WriteExpeditedRx(SdoWriteExpeditedRx),
    WriteExpeditedTx(SdoWriteExpeditedTx),
}

#[derive(Debug)]
pub struct SdoWriteExpeditedRx {
    pub index: u16,
    pub sub_index: u8,
    pub data: Box<[u8]>, // Data (up to 4 bytes, can be less or unused based on command)
}

impl SdoWriteExpeditedRx {
    fn encode(&self, frame: &mut socketcan::CanFrame) {
        let mut data = [0u8; 8];
        let mut scs = 0b00100000;
        let l = match self.data.len() {
            1 => 0b11,
            2 => 0b10,
            3 => 0b01,
            4 => 0b00,
            _ => unreachable!()
        };
        scs |= l << 2;
        scs |= 0b11;
        data[0] = scs;
        data[1..3].copy_from_slice(&self.index.to_le_bytes());
        data[3] = self.sub_index;
        data[4..4+self.data.len()].copy_from_slice(&self.data);
        frame.set_data(&data).unwrap();
    }

    fn decode(frame: &socketcan::CanFrame) -> Result<Self, CanOpenError> {
        let l = match (frame.data()[0] & 0b00001100) >> 2 {
            0b11 => 1,
            0b10 => 2,
            0b01 => 3,
            0b00 => 4,
            // this path is technically unreachable, it must be a regression
            _ => return Err(CanOpenError::ParseError("logic bug while decoding sdo".to_owned()))
            
        };
        let index = u16::from_le_bytes(
            frame.data()[1..3].try_into().map_err(|_| CanOpenError::ParseError("not enough data".to_owned()))
            ?);
        let sub_index = frame.data()[3];

        let mut data = Vec::with_capacity(l);
        data.extend_from_slice(&frame.data()[4..4+l]);
        Ok(Self {
            index,
            sub_index,
            data: data.into()
        })
    }
}

#[derive(Debug)]
pub struct SdoWriteExpeditedTx {
    pub index: u16,
    pub sub_index: u8,
}


impl SdoWriteExpeditedTx {
    fn encode(&self, frame: &mut socketcan::CanFrame) {
        let mut data = [0u8; 8];
        data[0] = 0b01100000;
        data[1..3].copy_from_slice(&self.index.to_le_bytes());
        data[3] = self.sub_index;
        frame.set_data(&data);
    }

    fn decode(frame: &socketcan::CanFrame) -> Result<Self, CanOpenError> {
        let index = u16::from_le_bytes(
            frame.data()[1..3].try_into().map_err(|_| CanOpenError::ParseError("not enough data".to_owned()))
            ?);
        let sub_index = frame.data()[3];

        Ok(Self {index, sub_index})
    }

}


/// SDO Command Specifier
/// SDOs let you read/write object dictionary keys
/// An expedited SDO message carries at most 4 bytes
/// Segmented SDOs are for sending more than 4 bytes
/// As usual in embedded, comms are from the perspective of the embedded device,
/// so download = client to server, upload = server to client.
///
/// When writing less than 4 bytes, you send a `InitiateDownload` RX,
/// the device ACKs it with `InitiateDownload` TX
///
/// When writing more than 4 bytes, you send an `InitiateDownload` RX
/// with the "segmented" flag set and the # of bytes you'll send in the data field.
/// The device still acks this with `InitiateDownload` TX.
/// After this, you send each segment with a `DownloadSegment`. The device still ACKs each segment.
/// The `DownloadSegment` TX can carry at most 8 bytes
///
/// Reading works symmetrically to writing
#[derive(Debug, PartialEq)]
enum SdoCs {
    DownloadSegment,
    InitiateDownload,
    InitiateUpload,
    UploadSegment,
    AbortTransfer,
    BlockUpload,
    BlockDownload,
}

impl SdoCs {
    pub fn from_byte(byte: u8, rxtx: Rxtx) -> Result<SdoCs, CanOpenError> {
        use SdoCs::*;
        let v = match (rxtx, byte >> 5) {
            // meaning of the command specifier is slightly different for Rx and Tx
            // thx: https://github.com/wireshark/wireshark/blob/master/epan/dissectors/packet-canopen.c#L511
            (Rxtx::RX, 0x00) => DownloadSegment,
            (Rxtx::RX, 0x01) => InitiateDownload,
            (Rxtx::RX, 0x02) => InitiateUpload,
            (Rxtx::RX, 0x03) => UploadSegment,

            (Rxtx::TX, 0x00) => UploadSegment,
            (Rxtx::TX, 0x01) => DownloadSegment,
            (Rxtx::TX, 0x02) => InitiateUpload,
            (Rxtx::TX, 0x03) => InitiateDownload,

            (_, 0x04) => AbortTransfer,
            (_, 0x05) => BlockUpload,
            (_, 0x06) => BlockDownload,
            _ => return Err(CanOpenError::ParseError(format!("bad client command specifier: {}", byte)))
        };
        Ok(v)
    }
    pub fn to_byte(&self, rxtx: Rxtx) -> u8 {
        match (rxtx, self) {
            (Rxtx::RX, SdoCs::DownloadSegment) => 0x00,
            (Rxtx::RX, SdoCs::InitiateDownload) => 0x01,
            (Rxtx::RX, SdoCs::InitiateUpload) => 0x02,
            (Rxtx::RX, SdoCs::UploadSegment) => 0x03,

            (Rxtx::TX, SdoCs::UploadSegment) => 0x00,
            (Rxtx::TX, SdoCs::DownloadSegment) => 0x01,
            (Rxtx::TX, SdoCs::InitiateUpload) => 0x02,
            (Rxtx::TX, SdoCs::InitiateDownload) => 0x03,

            (_, SdoCs::AbortTransfer) => 0x04,
            (_, SdoCs::BlockUpload) => 0x05,
            (_, SdoCs::BlockDownload) => 0x06,
        }
    }
}


impl Sdo {
    pub fn new_write(node_id: u8, index: u16, sub_index: u8, data: Box<[u8]>) -> Sdo {
        Sdo {
            node_id,
            rxtx: Rxtx::RX,
            command: SdoCmd::WriteExpeditedRx(SdoWriteExpeditedRx{index, sub_index, data}),
        }
    }
    pub fn new_write_resp(node_id: u8, index: u16, sub_index: u8) -> Sdo {
        Sdo {
            node_id,
            rxtx: Rxtx::TX,
            command: SdoCmd::WriteExpeditedTx(SdoWriteExpeditedTx{index, sub_index}),
        }
    }

}

impl FrameRW for Sdo {
    fn decode(frame: &socketcan::CanFrame) -> Result<Sdo, CanOpenError> {
        let data = frame.data();

        let id = id_as_raw_std(frame)?;
        if !(id >= 0x580 && id <= 0x5FF) && !(id >= 0x600 && id <= 0x67F) {
            return Err(CanOpenError::BadMessage(format!("{id} is not an SDO can id").to_owned())); // Not a valid SDO COB-ID
        }

        let node_id = (id & 0x7F) as u8;
        let rxtx = Rxtx::from_u16_sdo(id);

        let command_byte = data[0];
        let command_spec = SdoCs::from_byte(data[0], rxtx)?;
        let command = match (rxtx, command_spec) {
            (Rxtx::RX, SdoCs::InitiateDownload) => {
                    if command_byte & 0b10 == 0b10 { // expedited
                        SdoCmd::WriteExpeditedRx(SdoWriteExpeditedRx::decode(frame)?)
                    } else {
                        todo!() // yet to implement segmented writes
                    }
                },
            (Rxtx::TX, SdoCs::InitiateDownload) => {
                    if command_byte & 0b10 == 0b10 { // expedited
                        SdoCmd::WriteExpeditedTx(SdoWriteExpeditedTx::decode(frame)?)
                    } else {
                        todo!() // yet to implement segmented writes
                    }
                },
            _ => todo!() // yet to implement other sdo commands
        };
        let sdo = Sdo{
            node_id,
            command,
            rxtx,
        };
        Ok(sdo)
    }

    fn encode(&self, frame: &mut socketcan::CanFrame) {
        frame.set_id(u16_as_id((self.node_id as u16) + self.rxtx.to_u16_sdo()));
        match &self.command {
            SdoCmd::WriteExpeditedRx(inner) => inner.encode(frame),
            SdoCmd::WriteExpeditedTx(inner) => inner.encode(frame),
            _ => todo!() // yet to implement other sdo commands
        };

    }
}


#[derive(Debug, Copy, Clone)]
#[repr(u8)]
pub enum GuardStatus {
    Boot = 0x00,
    Stopped = 0x04,
    Operational = 0x05,
    PreOperational = 0x7F,
}

impl TryFrom<u8> for GuardStatus {
    type Error = String;
    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0x00 => Ok(GuardStatus::Boot),
            0x04 => Ok(GuardStatus::Stopped),
            0x05 => Ok(GuardStatus::Operational),
            0x7F => Ok(GuardStatus::PreOperational),
            _ => Err(format!("{value:x} not a valid guard status").to_owned())
        }
    }
}

#[binrw]
#[brw(little)]
#[derive(Debug)]
pub struct Guard {
    #[brw(ignore)]
    node_id: u8,

    #[br(temp)]
    #[bw(calc = (*status as u8)  | ((*toggle as u8) << 7))]
    raw_byte: u8,

    #[br(calc = raw_byte & 0x80 != 0)]
    #[bw(ignore)]
    toggle: bool,

    #[br(try_calc = (raw_byte & 0x7F).try_into())]
    #[bw(ignore)]
    status: GuardStatus,
}

impl Guard {
    pub fn new(node_id: u8, toggle: bool, status: GuardStatus) -> Self {
        Self {
            node_id, toggle, status
        }
    }
}

impl FrameRW for Guard {
    fn decode(frame: &socketcan::CanFrame) -> Result<Guard, CanOpenError> {
        let data = frame.data();
        if data.is_empty() {
            return Err(CanOpenError::ParseError("data too short".to_owned()));
        }

        let id = id_as_raw_std(frame)?;
        if !(0x700..=0x77F).contains(&id) {
            return Err(CanOpenError::BadMessage("wrong id".to_owned()));
        }
        Guard::read(&mut std::io::Cursor::new(&data)).map_err(|e| CanOpenError::ParseError(format!("no parse: {e}")))
    }

    fn encode(&self, frame: &mut socketcan::CanFrame) {
        frame.set_id(u16_as_id(0x700 + self.node_id as u16));
        let mut c = std::io::Cursor::new(Vec::new());
        self.write(&mut c).unwrap();
        frame.set_data(c.get_ref()).unwrap();
    }
}

/// Rxtx realtive to the device (aka server)
/// Data sent from your computer is probably Rx,
/// since the device is receiving it)
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rxtx {
    #[default] RX,
    TX,
}
impl Rxtx {
    pub fn to_u16_sdo(&self) -> u16 {
        match self {
            Rxtx::RX => 0x600,
            Rxtx::TX => 0x580,
        }
    }
    pub fn from_u16_sdo(id: u16) -> Self {
        if id & 0x780 == 0x580 { Rxtx::TX } else { Rxtx::RX }
    }
}

#[derive(Debug)]
pub struct Pdo {
    node_id: u8,
    pdo_index: u8, // PDO index (1 to 4)
    rxtx: Rxtx,
    data: Vec<u8>, // Data (1 to 8 bytes)
}

impl Pdo {
    pub fn new(node_id: u8, pdo_index: u8, data: &[u8]) -> Self {
        Self {node_id, pdo_index, rxtx: Rxtx::RX, data: data.to_owned()}
    }

}

impl FrameRW for Pdo {
    fn decode(frame: &socketcan::CanFrame) -> Result<Pdo, CanOpenError> {
        let id = id_as_raw_std(frame)?;
        let data = frame.data().to_vec();

        // Determine RX/TX and PDO index from the COB-ID
        let rxtx = if id & 0x80 == 0 {
            Rxtx::TX
        } else { Rxtx::RX };

        // this is a bit odd, RX indicies are offset by one
        let pdo_index = (((id & 0x700) >> 8) as u8) - if rxtx == Rxtx::RX { 1u8 } else { 0u8 }; 

        let node_id = (id & 0x7F) as u8;

        Ok(Pdo {
            pdo_index,
            rxtx,
            node_id,
            data,
        })
    }

    fn encode(&self, frame: &mut socketcan::CanFrame) {
        let id = (self.pdo_index as u16 + if self.rxtx == Rxtx::RX { 1 } else { 0 }) << 8;
        frame.set_id(u16_as_id(self.node_id as u16 + id));
        frame.set_data(&self.data);
    }
}

#[derive(Debug, Clone, PartialEq)]
enum TpdoType {
    AcyclicSynchronous,
    CyclicSynchronous(u8), // The value represents the SYNC interval divisor
    SynchronousRtrOnly,
    AsynchronousRtrOnly,
    Asynchronous,
    // Add more types or specific variants if needed
}

#[derive(Debug)]
pub struct Sync;

impl FrameRW for Sync {
    fn decode(frame: &socketcan::CanFrame) -> Result<Sync, CanOpenError> {
        let id = id_as_raw_std(frame)?;
        if id != 0x80 {
            return Err(todo!()); // Not a valid Sync COB-ID
        }

        if !frame.data().is_empty() {
            return Err(todo!()); // Sync message should not have any data
        }

        Ok(Sync {})
    }

    fn encode(&self, frame: &mut socketcan::CanFrame) {
        frame.set_id(u16_as_id(0x80));
        frame.set_data(&[]);
    }
}

#[derive(Debug)]
pub enum Message {
    Nmt(Nmt),
    Sync(Sync),
    Emergency(Emergency),
    Pdo(Pdo),
    Sdo(Sdo),
    Guard(Guard),
}

use thiserror::Error;

#[derive(Error, Debug)]
pub enum CanOpenError {
    #[error("FrameRW protocl is not {0}")]
    BadMessage(String),

    #[error("Connection error: {0}")]
    ConnectionError(String),

    #[error("CAN version mismatch: {0}")]
    CanVersion(String),

    #[error("Parse error: {0}")]
    ParseError(String),

    #[error("Unknown message type with COB-ID: {0}")]
    UnknownFrameRWType(u32),

    #[error("IO Error: {0}")]
    IOError(std::io::Error),
}


#[derive(Debug)]
pub struct Conn {
    socket: socketcan::CanSocket,
}
use socketcan::Socket;

impl Conn {
    pub fn new(interface_name: &str) -> Result<Self, CanOpenError> {
        let socket = socketcan::CanSocket::open(interface_name).expect("no iface");
        Ok(Conn { socket })
    }

    pub fn recv(&self) -> Result<Message, CanOpenError> {
        let frame = self.socket.read_frame()
            .map_err(|e| CanOpenError::ConnectionError(e.to_string()))?;
        Self::decode(&frame)
    }

    pub fn send(&self, message: Message) -> Result<(), CanOpenError> {
        let mut frame = socketcan::CanFrame::new(socketcan::Id::Standard(socketcan::StandardId::new(0).unwrap()), &[]).unwrap();
        match message {
            Message::Sdo(sdo) => sdo.encode(&mut frame),
            Message::Pdo(pdo) => pdo.encode(&mut frame),
            Message::Sync(sync) => sync.encode(&mut frame),
            Message::Nmt(nmt) => nmt.encode(&mut frame),
            Message::Emergency(emergency) => emergency.encode(&mut frame),
            Message::Guard(guard) => guard.encode(&mut frame),
            _ => todo!()
        }
        self.socket.write_frame(&frame).map_err(CanOpenError::IOError)
    }

    fn decode(frame: &socketcan::CanFrame) -> Result<Message, CanOpenError> {
        let id = id_as_raw_std(&frame).unwrap();
        // can_id is node_id + protocol_id (same as function id)
        // can_ids are always <128
        // mask out lowest 7 bits to just get the protocol_id
        let protocol_id = id & 0xFF80;
        // apply the opposite mask for node_id
        let node_id = id & 0x007F;
        let p = match protocol_id {
            0x000 => Message::Nmt(Nmt::decode(frame)?),
            0x080 if node_id == 0 => Message::Sync(Sync::decode(frame)?),
            0x080 => Message::Emergency(Emergency::decode(frame)?),
            0x180..=0x500 => Message::Pdo(Pdo::decode(frame)?),
            0x580..=0x600 => Message::Sdo(Sdo::decode(frame)?),
            0x700 => Message::Guard(Guard::decode(frame)?),
            _ => todo!()
        };
        Ok(p)

    }
}


