/// WIP below - node impl

use std::collections::HashMap;
type ObjectDictionary = HashMap<u16, Vec<u8>>;

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
                    if sdo.sdo_type == TransferSize::Expedited && (sdo.command & 0x2F) == 0x20 {
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
