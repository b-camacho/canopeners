use socketcan::Id;
use socketcan::Frame;
use socketcan::EmbeddedFrame;


// CANOpen ids can only be standard (11bit), not extended (29bit)
// returns None when CanFrame has extended ID
fn id_as_raw_std(frame: &socketcan::CanFrame) -> Option<u16> {
    if let Id::Standard(sid) = frame.id() {
        Some(sid.as_raw())
    } else {
        None
    }
}

fn mk_can_frame(cob_id: u16, data: &[u8]) -> socketcan::CanDataFrame {
    socketcan::CanDataFrame::new(socketcan::StandardId::new(cob_id).unwrap(), data).unwrap()
}

#[derive(Clone, Debug)]
struct Nmt {
    nmt_function: NmtFunction,
    target_node: u8,
}

impl Nmt {
    fn parse_from_frame(frame: &socketcan::CanFrame) -> Option<Self> {
        if id_as_raw_std(frame)? != 0x000 || frame.dlc() != 2 {
            return None;
        }

        let data = frame.data();
        Some(Self {
            nmt_function: NmtFunction::from_byte(data[0])?,
            target_node: data[1],
        })
    }
}

#[derive(Clone, Debug)]
enum NmtFunction {
    EnterOperational,
    EnterStop,
    EnterPreOperational,
    ResetNode,
    ResetCommunication,
}

impl NmtFunction {
    fn from_byte(byte: u8) -> Option<Self> {
        match byte {
            0x01 => Some(NmtFunction::EnterOperational),
            0x02 => Some(NmtFunction::EnterStop),
            0x80 => Some(NmtFunction::EnterPreOperational),
            0x81 => Some(NmtFunction::ResetNode),
            0x82 => Some(NmtFunction::ResetCommunication),
            _ => None,
        }
    }
}


#[derive(Debug)]
struct Emergency {
    node_id: u8,
    error_code: u16,
    error_register: u8,
    vendor_specific_data: [u8; 5], // Assuming 5 bytes of vendor-specific data
}

impl Emergency {
    fn parse_from_frame(frame: &socketcan::CanFrame) -> Option<Self> {
        let data = frame.data();
        if data.len() < 8 {
            return None; // Not a valid Emergency message (less than 8 bytes)
        }

        let error_code = u16::from_be_bytes([data[0], data[1]]);
        let error_register = data[2];
        let mut vendor_specific_data = [0u8; 5];
        vendor_specific_data.copy_from_slice(&data[3..8]);

        Some(Emergency {
            node_id: (id_as_raw_std(frame)? - 0x080) as u8,
            error_code,
            error_register,
            vendor_specific_data,
        })
    }
}


#[derive(Debug)]
enum SdoType {
    Expedited,
    Segmented,
}

#[derive(Debug)]
struct Sdo {
    node_id: u8, // Derived from the header
    command: u8,
    index: u16,
    sub_index: u8,
    data: [u8; 4], // Data (up to 4 bytes, can be less or unused based on command)
    sdo_type: SdoType,
}

impl Sdo {
    fn parse_from_frame(frame: &socketcan::CanFrame) -> Option<Self> {
        let data = frame.data();
        if data.len() < 8 {
            return None; // Not a valid SDO message (less than 8 bytes)
        }

        let id = id_as_raw_std(frame)?;
        if !(id >= 0x580 && id <= 0x5FF) && !(id >= 0x600 && id <= 0x67F) {
            return None; // Not a valid SDO COB-ID
        }

        let node_id = (id & 0x7F) as u8;
        let index = u16::from_le_bytes([data[1], data[2]]);
        let sub_index = data[3];
        let mut sdo_data = [0u8; 4];
        sdo_data.copy_from_slice(&data[4..8]);

        // Determine the SDO type (expedited or segmented) from the command byte
        let sdo_type = if data[0] & 0x02 != 0 { SdoType::Expedited } else { SdoType::Segmented };

        Some(Sdo {
            node_id,
            command: data[0],
            index,
            sub_index,
            data: sdo_data,
            sdo_type,
        })
    }
}


#[derive(Debug)]
enum GuardStatus {
    Boot,
    Stopped,
    Operational,
    PreOperational,
}

impl GuardStatus {
    fn from_byte(byte: u8) -> Option<Self> {
        match byte {
            0x00 => Some(GuardStatus::Boot),
            0x04 => Some(GuardStatus::Stopped),
            0x05 => Some(GuardStatus::Operational),
            0x7F => Some(GuardStatus::PreOperational),
            _ => None,
        }
    }
}

#[derive(Debug)]
struct Guard {
    node_id: u8, // Derived from the header
    toggle_bit: bool,
    status: GuardStatus,
}

impl Guard {
    fn parse_from_frame(frame: &socketcan::CanFrame) -> Option<Self> {
        let data = frame.data();
        if data.len() < 1 {
            return None;
        }

        let id = id_as_raw_std(frame)?;
        if id < 0x700 || id > 0x77F {
            return None;
        }

        let node_id = (id - 0x700) as u8;
        let toggle_bit = data[0] & 0x80 != 0;
        let status_byte = data[0] & 0x7F;

        Some(Guard {
            node_id,
            toggle_bit,
            status: GuardStatus::from_byte(status_byte)?,
        })
    }
}

#[derive(Debug)]
enum Rxtx {
    RX,
    TX,
}

#[derive(Debug)]
struct Pdo {
    pdo_index: u8, // PDO index (1 to 4)
    rxtx: Rxtx,
    node_id: u8, // Derived from the header
    data: Vec<u8>, // Data (1 to 8 bytes)
}

impl Pdo {
    fn parse_from_frame(frame: &socketcan::CanFrame) -> Option<Self> {
        let id = id_as_raw_std(frame)?;
        let data = frame.data().to_vec();

        // Determine RX/TX and PDO index from the COB-ID
        let (rxtx, pdo_index) = match id {
            0x180..=0x1FF => (Rxtx::TX, (id - 0x180) as u8),
            0x200..=0x27F => (Rxtx::RX, (id - 0x200) as u8),
            _ => return None, // Not a valid PDO COB-ID
        };

        let node_id = (id & 0x7F) as u8;

        Some(Pdo {
            pdo_index,
            rxtx,
            node_id,
            data,
        })
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
struct Sync;

impl Sync {
    fn parse_from_frame(frame: &socketcan::CanFrame) -> Option<Self> {
        let id = id_as_raw_std(frame)?;
        if id != 0x80 {
            return None; // Not a valid Sync COB-ID
        }

        if !frame.data().is_empty() {
            return None; // Sync message should not have any data
        }

        Some(Sync {})
    }
}

enum Message {
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
    #[error("Connection error: {0}")]
    ConnectionError(String),

    #[error("Parse error: {0}")]
    ParseError(String),

    #[error("Unknown message type with COB-ID: {0}")]
    UnknownMessageType(u32),

    #[error("IO Error: {0}")]
    IOError(std::io::Error),
}


#[derive(Debug)]
struct Conn {
    socket: socketcan::CanSocket,
}
use socketcan::Socket;

impl Conn {
    fn new(interface_name: &str) -> Result<Self, CanOpenError> {
        let socket = socketcan::CanSocket::open(interface_name).expect("no iface");
        Ok(Conn { socket })
    }

    fn recv(&self) -> Result<Message, CanOpenError> {
        let frame = self.socket.read_frame()
            .map_err(|e| CanOpenError::ConnectionError(e.to_string()))?;

        let message = if let Some(nmt) = Nmt::parse_from_frame(&frame) {
            Some(Message::Nmt(nmt))
        } else if let Some(sync) = Sync::parse_from_frame(&frame) {
            Some(Message::Sync(sync))
        } else if let Some(emergency) = Emergency::parse_from_frame(&frame) {
            Some(Message::Emergency(emergency))
        } else if let Some(sdo) = Sdo::parse_from_frame(&frame) {
            Some(Message::Sdo(sdo))
        } else if let Some(guard) = Guard::parse_from_frame(&frame) {
            Some(Message::Guard(guard))
        } else { Pdo::parse_from_frame(&frame).map(Message::Pdo) };
        message.ok_or(CanOpenError::ParseError("frame {frame:?} did not parse as any canopen function".to_owned()))
    }
}

use std::collections::HashMap;

#[derive(Debug, Clone)]
struct ObjectDictionary {
    pub data: HashMap<u16, Vec<u8>>,
}

impl ObjectDictionary {
    fn new() -> Self {
        ObjectDictionary {
            data: HashMap::new(),
        }
    }

}

#[derive(Debug, Clone)]
struct Node {
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
                        // Handle expedited SDO write
                        // Here, sdo.index is the object dictionary index to write to
                        // sdo.data contains the data to write

                        // Update the object dictionary
                        self.object_dictionary.insert(sdo.index, sdo.data.to_vec());

                        // Respond to the SDO write request
                        // The specifics of this response depend on your CANOpen implementation
                        // Typically, a confirmation message is sent back
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

   fn send_sdo_confirmation(&self, node_id: u8, index: u16, sub_index: u8) -> io::Result<()> {
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
            
        self.conn.socket.write_frame_insist(&frame)?;

        Ok(())
    }

   fn extract_tpdo_configs(&self) -> Vec<(u32, TpdoType, Vec<(u16, u8, u8)>)> {
        let mut tpdo_configs = Vec::new();

        // Assuming TPDOs are configured at standard indexes
        // Adjust these indexes according to your specific CANOpen implementation
        const TPDO_CONFIG_START_INDEX: u16 = 0x1A00;
        const NUMBER_OF_TPDOS: u16 = 4; // Adjust based on how many TPDOs are supported

        for i in 0..NUMBER_OF_TPDOS {
            let pdo_index = TPDO_CONFIG_START_INDEX + i;
            if let Some((cob_id, tpdo_type, mappings)) = self.parse_pdo_config(pdo_index) {
                tpdo_configs.push((cob_id, tpdo_type, mappings));
            }
        }

        tpdo_configs
    }

    fn parse_pdo_config(&self, pdo_index: u16) -> Option<(u32, TpdoType, Vec<(u16, u8, u8)>)> {
        // COB-ID for the PDO, defaulting to 0x180 + node_id
        let cob_id = self.object_dictionary.get(pdo_index).unwrap_or(&vec![0x00, 0x01]).clone();

        // Parse the COB-ID (assuming it's 4 bytes, little endian)
        let cob_id = u32::from_le_bytes([cob_id[0], cob_id[1], cob_id[2], cob_id[3]]);

        // Get the transmission type
        let type_field = self.object_dictionary.get(pdo_index + 1).unwrap_or(&vec![0x00]).clone();
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
            if let Some(data) = self.object_dictionary.get(*index) {
                // Extract the specified bytes from the object dictionary entry
                // Placeholder: Implement logic to handle sub-indices and lengths
                tpdo_data.extend_from_slice(data);
            }
        }
        tpdo_data
    }

    fn send_tpdo(&self, cob_id: u32, data: &[u8]) -> Result<(), CanOpenError> {
        if data.len() > 8 {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "TPDO data exceeds 8 bytes"));
        }

        let frame = mk_can_frame(cob_id as u16, data);

        self.conn.socket.write_frame_insist(&frame);

        Ok(())
    }
}




