use crate::CanOpenError;
use std::collections::HashMap;
use binrw::{
    binrw,
    BinRead,
    BinWrite,
};

// All CANOpen operations are expressed in terms of Object Dictionary operations
// SDOs read/write OD records in a request/response format
// PDOs cause OD records to be transmitted at intervals, upon events etc
// Crucially, the OD also configures the behavior of the node
// For example, what data should be transmitted over PDO is 
// also configured in the object dictionary
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
struct OdIndex {
    index: u16,
    sub_index: u8,
}

impl OdIndex {
    pub fn new(index: u16, sub_index: u8) -> Self {
        Self {index, sub_index}
    }

}

struct Od {
    // spec says object dicionary records can be "any data"
    // for now just store arbitrary bytes
    inner: HashMap<OdIndex, Vec<u8>>

}
//fn merge<K, V>(d1: HashMap<K, V>, d2: HashMap<K, V>) where K : std::hash::Hash {
//    for (k, v) in d2 {
//        d1.insert()
//    }
//
//}

impl Od {
    pub fn apply_chunk(&mut self, chunk: OdChunk) {
        self.inner.extend(chunk);
    }
}
// Used for expressing mutations to the dictionary
// For example, serializing a TpdoConfig returns an OdChunk
// You can then apply the chunk to a local dictionary
// Or send it to a remote node's dictionary via SDO
type OdChunk = HashMap<OdIndex, Vec<u8>>;



#[binrw]
#[brw(little)]
#[derive(Debug)]
struct TpdoConfig {
    #[brw(ignore)]
    mappings: Vec<TpdoConfigMapping>,

    cob_id: u32,
    #[br(temp)]
    #[bw(calc = freq.to_byte())]
    freq_raw: u8,
    #[br(try_calc = TpdoConfigFreq::from_byte(freq_raw))]
    #[bw(ignore)]
    freq: TpdoConfigFreq,
    inhibit_time: u16,
    event_timer: u16,
}

impl TpdoConfig {
    pub fn from_entries(lower: &[[u8; 4]], upper: &[u8; 8]) -> Self {
        let mappings = lower.iter().map(|d| TpdoConfigMapping::decode(d)).collect::<_>();
        let mut config = TpdoConfig::read(&mut std::io::Cursor::new(upper)).unwrap();
        config.mappings = mappings;
        config
    }

    pub fn to_entries(&self) -> (Vec<[u8; 4]>, [u8; 8]) {
        let mut config_data = [0; 8];
        self.write(&mut std::io::Cursor::new(config_data));

        let mappings = self.mappings.iter().map(|e| e.encode()).collect::<Vec<_>>();
        (mappings, config_data)
    }

    pub fn to_od(&self) -> OdChunk {
        let (lower, upper) = self.to_entries();
        let mut h = HashMap::new();
        h.insert(OdIndex::new(0x1a00, 0), lower.len().to_le_bytes().to_vec());
        for (idx, l) in lower.iter().enumerate() {
            h.insert(OdIndex::new(0x1a00, idx as u8 + 1), l.to_vec());
        }
        h.insert(OdIndex::new(0x1800, 1), upper.to_vec());
        h
    }

    pub fn from_od(od: Od) -> Vec<Self> {
        for base_idx in 0x1a00..0x1800 {
            if let Some(len) = od.inner.get(OdIndex(base_idx, 0)) {


            }

        }
        

    }
}

#[binrw]
#[brw(little)]
#[derive(Debug)]
struct TpdoConfigMapping {
    index: u16,
    sub_index: u8,
    length: u8,
}

impl TpdoConfigMapping {
    pub fn decode(data: &[u8]) -> Self {
        Self::read(&mut std::io::Cursor::new(data)).unwrap()
    }
    pub fn encode(&self) -> [u8; 4] {
        let mut c = std::io::Cursor::new([0; 4]);
        self.write(&mut c);
        c.into_inner()
    }

}

#[derive(Debug, Clone, PartialEq)]
enum TpdoConfigFreq {
    AcyclicSynchronous,
    CyclicSynchronous(u8), // The value represents the SYNC interval divisor
    SynchronousRtrOnly,
    AsynchronousRtrOnly,
    Asynchronous,
}

impl TpdoConfigFreq {
    fn to_byte(&self) -> u8 {
        match self {
            TpdoConfigFreq::AcyclicSynchronous => 0,
            TpdoConfigFreq::CyclicSynchronous(interval) => *interval,
            TpdoConfigFreq::SynchronousRtrOnly => 252,
            TpdoConfigFreq::AsynchronousRtrOnly => 253,
            TpdoConfigFreq::Asynchronous => 254,
        }
    }

    fn from_byte(byte: u8) -> Result<Self, CanOpenError> {
        match byte {
            0 => Ok(TpdoConfigFreq::AcyclicSynchronous),
            1..=240 => Ok(TpdoConfigFreq::CyclicSynchronous(byte)),
            252 => Ok(TpdoConfigFreq::SynchronousRtrOnly),
            253 => Ok(TpdoConfigFreq::AsynchronousRtrOnly),
            254 | 255 => Ok(TpdoConfigFreq::Asynchronous),
            _ => Err(CanOpenError::ParseError(format!("bad TpdoConfigFreq: {}", byte))),
        }
    }
}
