use socketcan::Id;
use socketcan::Frame;
use socketcan::EmbeddedFrame;
use binrw::{
    binrw,
    BinRead,
    BinWrite,
};


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
    nmt_function: NmtFunction,
    target_node: u8,
}

impl FrameRW for Nmt {
    fn decode(frame: &socketcan::CanFrame) -> Result<Nmt, CanOpenError> {
        let mut c = std::io::Cursor::new(frame.data());
        Nmt::read(&mut c).map_err(|binrw_err| CanOpenError::ParseError(binrw_err.to_string()))

        //Some(Self {
        //    nmt_function: NmtFunction::from_byte(data[0])?,
        //    target_node: data[1],
        //})
    }

    fn encode(&self, frame: &mut socketcan::CanFrame) {
        frame.set_id(u16_as_id(0x000));
        let mut c = std::io::Cursor::new(Vec::new());
        self.write(&mut c);
        frame.set_data(c.get_ref());
    }

    //fn id(&self) -> Option<u16> {
    //    None
    //}
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


#[derive(Debug)]
#[binrw]
#[brw(little)]
pub struct Emergency {
    #[brw(ignore)] node_id: u8,
    error_code: u16,
    error_register: u8,
    vendor_specific_data: [u8; 5], // Assuming 5 bytes of vendor-specific data
}

impl FrameRW for Emergency {
    fn decode(frame: &socketcan::CanFrame) -> Result<Emergency, CanOpenError> {
        let data = frame.data();
        if data.len() < 8 {
            return Err(CanOpenError::ParseError("not a valid Emergency message, need at least 8 bytes".to_owned()));
        }
        let mut res = Emergency::read(&mut std::io::Cursor::new(data));

        let error_code = u16::from_be_bytes([data[0], data[1]]);
        let error_register = data[2];
        let mut vendor_specific_data = [0u8; 5];
        vendor_specific_data.copy_from_slice(&data[3..8]);

        Ok(Emergency {
            node_id: (id_as_raw_std(frame)? - 0x080) as u8,
            error_code,
            error_register,
            vendor_specific_data,
        })
    }

    fn encode(&self, frame: &mut socketcan::CanFrame) {
        frame.set_id(u16_as_id(self.node_id as u16));
        let mut c = std::io::Cursor::new(Vec::new());
        self.write(&mut c);
        frame.set_data(c.get_ref());
    }
}


#[derive(Debug, PartialEq, Eq)]
pub enum SdoType {
    Expedited,
    Segmented,
}
impl SdoType {
    pub(crate) fn from_byte(command: u8) -> Self {
        if command & 0x02 != 0 { SdoType::Expedited } else { SdoType::Segmented }
    }

}

#[binrw]
#[brw(little)]
#[derive(Debug)]
pub struct Sdo {
    #[brw(ignore)]
    node_id: u8, // Derived from the header

    command: u8,
    index: u16,
    sub_index: u8,
    data: [u8; 4], // Data (up to 4 bytes, can be less or unused based on command)
    #[bw(ignore)]
    #[br(calc = SdoType::from_byte(command))]
    sdo_type: SdoType,
}

impl Sdo {
    pub fn new(node_id: u8, command: u8, index: u16, sub_index: u8, data: &[u8]) {
        Sdo {
            node_id,
            command,
            index,
            sub_index,
            data,
            sdo_type: SdoType::from_byte(command)

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
        let index = u16::from_le_bytes([data[1], data[2]]);
        let sub_index = data[3];
        let mut sdo_data = [0u8; 4];
        sdo_data.copy_from_slice(&data[4..8]);

        // Determine the SDO type (expedited or segmented) from the command byte
        let sdo_type = if data[0] & 0x02 != 0 { SdoType::Expedited } else { SdoType::Segmented };

        Ok(Sdo {
            node_id,
            command: data[0],
            index,
            sub_index,
            data: sdo_data,
            sdo_type,
        })
    }

    fn encode(&self, frame: &mut socketcan::CanFrame) {
        frame.set_id(u16_as_id(self.node_id.into()));
        let mut c = std::io::Cursor::new(Vec::new());
        self.write(&mut c);
        frame.set_data(c.get_ref());
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
    node_id: u8, // Derived from the header

    #[br(temp)]
    #[bw(calc = (*status as u8)  | ((*toggle_bit as u8) << 7))]
    raw_byte: u8,

    #[br(calc = raw_byte & 0x80 != 0)]
    #[bw(ignore)]
    toggle_bit: bool,

    #[br(try_map = |x: u8| (x & 0x7F).try_into())]
    #[bw(ignore)]
    status: GuardStatus,
}

impl FrameRW for Guard {
    fn decode(frame: &socketcan::CanFrame) -> Result<Guard, CanOpenError> {
        let data = frame.data();
        if data.len() < 1 {
            return Err(CanOpenError::ParseError("data too short".to_owned()));
        }

        let id = id_as_raw_std(frame)?;
        if id < 0x700 || id > 0x77F {
            return Err(CanOpenError::BadMessage("wrong id".to_owned()));
        }
        Guard::read(&mut std::io::Cursor::new(&data)).map_err(|e| CanOpenError::ParseError(format!("no parse: {e}")))
    }

    fn encode(&self, frame: &mut socketcan::CanFrame) {
        frame.set_id(u16_as_id(self.node_id as u16));
        let mut c = std::io::Cursor::new(Vec::new());
        self.write(&mut c);
        frame.set_data(c.get_ref());
    }
}

#[derive(Debug, PartialEq, Eq)]
enum Rxtx {
    RX,
    TX,
}

#[derive(Debug)]
pub struct Pdo {
    pdo_index: u8, // PDO index (1 to 4)
    rxtx: Rxtx,
    node_id: u8, // Derived from the header
    data: Vec<u8>, // Data (1 to 8 bytes)
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
        let pdo_index = ((id & 0x700) as u8) + if rxtx == Rxtx::RX { 1 } else { 0 }; 

        let node_id = (id & 0x7F) as u8;

        Ok(Pdo {
            pdo_index,
            rxtx,
            node_id,
            data,
        })
    }

    fn encode(&self, frame: &mut socketcan::CanFrame) {
        frame.set_id(u16_as_id(self.node_id as u16));
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
        //match p {
        //    Message::Nmt => { 
        //        Message::Nmt(Nmt::decode(&frame)?);
        //    },
        //    Message::Sync => Sync::decode(&frame).map(Box::new),
        //    //Message::Emergency => Emergency::decode(&frame).map(Box::new),
        //    //Message::Pdo => Pdo::decode(&frame).map(Box::new),
        //    //Message::Sdo => Sdo::decode(&frame).map(Box::new),
        //    //Message::Guard => Guard::decode(&frame).map(Box::new),
        //};


        //let message = if let Some(nmt) = Nmt::decode(&frame) {
        //    Some(FrameRW::Nmt(nmt))
        //} else if let Some(sync) = Sync::decode(&frame) {
        //    Some(FrameRW::Sync(sync))
        //} else if let Some(emergency) = Emergency::decode(&frame) {
        //    Some(FrameRW::Emergency(emergency))
        //} else if let Some(sdo) = Sdo::decode(&frame) {
        //    Some(FrameRW::Sdo(sdo))
        //} else if let Some(guard) = Guard::decode(&frame) {
        //    Some(FrameRW::Guard(guard))
        //} else { Pdo::decode(&frame).map(FrameRW::Pdo) };
        //message.ok_or(CanOpenError::ParseError("frame {frame:?} did not decode as any canopen function".to_owned()))
    }

    pub fn send(&self, message: Message) -> Result<(), CanOpenError> {
        let mut frame = socketcan::CanFrame::new(socketcan::Id::Standard(socketcan::StandardId::new(0).unwrap()), &[]).unwrap();
        match message {
            Message::Sdo(sdo) => sdo.encode(&mut frame),
            Message::Sync(sync) => sync.encode(&mut frame),
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

use std::collections::HashMap;


type ObjectDictionary = HashMap<u16, Vec<u8>>;
//#[derive(Debug, Clone)]
//struct ObjectDictionary {
//    pub data: HashMap<u16, Vec<u8>>,
//}
//
//impl ObjectDictionary {
//    fn new() -> Self {
//        ObjectDictionary {
//            data: HashMap::new(),
//        }
//    }
//
//}

#[derive(Debug)]
pub struct Node {
    conn: Conn,
    object_dictionary: ObjectDictionary,
}

impl Node {
    fn new(conn: Conn, object_dictionary: ObjectDictionary) -> Self {
        Node {
            conn,
            object_dictionary,
        }
    }

    fn serve(&mut self) {
        loop {
            match self.conn.recv() {
                Ok(Message::Sdo(sdo)) => {
                    if sdo.sdo_type == SdoType::Expedited && (sdo.command & 0x2F) == 0x20 {
                        self.object_dictionary.insert(sdo.index, sdo.data.to_vec());
                        self.send_sdo_confirmation(sdo.node_id, sdo.index, sdo.sub_index);
                    }
                },
                Err(e) => {
                    // Handle error, possibly log it or break the loop
                    eprintln!("Error receiving message: {:?}", e);
                },
                _ => {
                    // Handle other message types or do nothing
                }
            }
        }
    }

   fn send_sdo_confirmation(&self, node_id: u8, index: u16, sub_index: u8) {
        // Create an SDO confirmation frame
        // For an expedited write confirmation, the command specifier is generally 0x60 combined with the sub command
        let command_specifier = 0x60;

        // Construct the data for the frame: command + index + sub_index + empty data
        let mut data = [0u8; 8];
        data[0] = command_specifier;
        data[1..3].copy_from_slice(&index.to_le_bytes());
        data[3] = sub_index;

        // The COB-ID for SDO Transmit is 0x580 + node ID
        let cob_id = 0x580 + node_id as u16;

        // Create the frame and send it
        let frame = mk_can_frame(cob_id, &data);
            
        self.conn.socket.write_frame_insist(&frame);

    }

   fn extract_tpdo_configs(&self) -> Vec<(u32, TpdoType, Vec<(u16, u8, u8)>)> {
        let mut tpdo_configs = Vec::new();

        // Assuming TPDOs are configured at standard indexes
        // Adjust these indexes according to your specific CANOpen implementation
        const TPDO_CONFIG_START_INDEX: u16 = 0x1A00;
        const NUMBER_OF_TPDOS: u16 = 4; // Adjust based on how many TPDOs are supported

        for i in 0..NUMBER_OF_TPDOS {
            let pdo_index = TPDO_CONFIG_START_INDEX + i;
            if let Some((cob_id, tpdo_type, mappings)) = self.decode_pdo_config(pdo_index) {
                tpdo_configs.push((cob_id, tpdo_type, mappings));
            }
        }

        tpdo_configs
    }

    fn decode_pdo_config(&self, pdo_index: u16) -> Option<(u32, TpdoType, Vec<(u16, u8, u8)>)> {
        // COB-ID for the PDO, defaulting to 0x180 + node_id
        let cob_id = self.object_dictionary.get(&pdo_index).unwrap_or(&vec![0x00, 0x01]).clone();

        // Parse the COB-ID (assuming it's 4 bytes, little endian)
        let cob_id = u32::from_le_bytes([cob_id[0], cob_id[1], cob_id[2], cob_id[3]]);

        // Get the transmission type
        let type_field = self.object_dictionary.get(&(pdo_index + 1)).unwrap_or(&vec![0x00]).clone();
        let tpdo_type = match type_field[0] {
            0 => TpdoType::AcyclicSynchronous,
            1..=240 => TpdoType::CyclicSynchronous(type_field[0]),
            252 => TpdoType::SynchronousRtrOnly,
            253 => TpdoType::AsynchronousRtrOnly,
            254..=255 => TpdoType::Asynchronous,
            _ => return None,
        };

        // Parse PDO mapping (assuming it starts from sub-index 1)
        // Each entry is 4 bytes long: 2 bytes for index, 1 byte for subindex, 1 byte for length
        let mut mappings = Vec::new();
        for i in 1..=8 {
            if let Some(entry) = self.object_dictionary.get(&(pdo_index + 0x200 + i)) {
                if entry.len() == 4 {
                    let index = u16::from_le_bytes([entry[0], entry[1]]);
                    let sub_index = entry[2];
                    let length = entry[3];
                    mappings.push((index, sub_index, length));
                }
            }
        }

        Some((cob_id, tpdo_type, mappings))
    }

    fn handle_sync(&self) {
        for (cob_id, tpdo_type, mappings) in &self.extract_tpdo_configs() {
            if self.should_send_tpdo(tpdo_type) {
                let tpdo_data = self.construct_tpdo_data(mappings);
                self.send_tpdo(*cob_id, &tpdo_data);
            }
        }
    }

    fn should_send_tpdo(&self, tpdo_type: &TpdoType) -> bool {
        // Determine if a TPDO should be sent based on its type and current conditions
        match tpdo_type {
            TpdoType::CyclicSynchronous(sync_rate) => {
                // Implement logic for cyclic synchronous TPDOs
                // For example, send on every SYNC message or based on a divisor of SYNC rate
                true // Placeholder, implement actual logic
            }
            TpdoType::Asynchronous => {
                // Implement logic for asynchronous TPDOs
                false // Placeholder, implement actual logic
            }
            // Handle other TPDO types
            _ => false,
        }
    }

    fn construct_tpdo_data(&self, mappings: &[(u16, u8, u8)]) -> Vec<u8> {
        // Construct the TPDO data based on the mappings
        let mut tpdo_data = Vec::new();
        for (index, sub_index, length) in mappings {
            if let Some(data) = self.object_dictionary.get(index) {
                // Extract the specified bytes from the object dictionary entry
                // Placeholder: Implement logic to handle sub-indices and lengths
                tpdo_data.extend_from_slice(data);
            }
        }
        tpdo_data
    }

    fn send_tpdo(&self, cob_id: u32, data: &[u8]) -> Result<(), CanOpenError> {
        let frame = mk_can_frame(cob_id as u16, data);

        self.conn.socket.write_frame_insist(&frame);

        Ok(())
    }
}




