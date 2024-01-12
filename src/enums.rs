use crate::CanOpenError;

#[derive(Clone, Debug)]
pub enum EmergencyErrorCode {
    ErrorResetOrNoError,
    GenericError,
    Current,
    CurrentInputSide,
    CurrentInsideDevice,
    CurrentOutputSide,
    Voltage,
    MainsVoltage,
    VoltageInsideDevice,
    OutputVoltage,
    Temperature,
    AmbientTemperature,
    DeviceTemperature,
    DeviceHardware,
    DeviceSoftware,
    InternalSoftware,
    UserSoftware,
    DataSet,
    AdditionalModules,
    Monitoring,
    Communication,
    CommunicationCanOverrun,
    CommunicationErrorPassiveMode,
    CommunicationLifeGuardError,
    CommunicationRecoveredBusOff,
    CommunicationCanIdCollision,
    ProtocolError,
    ProtocolErrorPdoLength,
    ProtocolErrorPdoLengthExceeded,
    ProtocolErrorDamMpdo,
    ProtocolErrorUnexpectedSyncLength,
    ProtocolErrorRpdoTimeout,
    ExternalError,
    AdditionalFunctions,
    DeviceSpecific,
}

impl EmergencyErrorCode {
    pub fn decode(code: u16) -> Result<Self, CanOpenError> {
        match code {
            0x8110 => Some(Self::CommunicationCanOverrun),
            0x8120 => Some(Self::CommunicationErrorPassiveMode),
            0x8130 => Some(Self::CommunicationLifeGuardError),
            0x8140 => Some(Self::CommunicationRecoveredBusOff),
            0x8150 => Some(Self::CommunicationCanIdCollision),
            0x8210 => Some(Self::ProtocolErrorPdoLength),
            0x8220 => Some(Self::ProtocolErrorPdoLengthExceeded),
            0x8230 => Some(Self::ProtocolErrorDamMpdo),
            0x8240 => Some(Self::ProtocolErrorUnexpectedSyncLength),
            0x8250 => Some(Self::ProtocolErrorRpdoTimeout),
            0x2100..=0x21FF => Some(Self::CurrentInputSide),
            0x2200..=0x22FF => Some(Self::CurrentInsideDevice),
            0x2300..=0x23FF => Some(Self::CurrentOutputSide),
            0x3100..=0x31FF => Some(Self::MainsVoltage),
            0x3200..=0x32FF => Some(Self::VoltageInsideDevice),
            0x3300..=0x33FF => Some(Self::OutputVoltage),
            0x4100..=0x41FF => Some(Self::AmbientTemperature),
            0x4200..=0x42FF => Some(Self::DeviceTemperature),
            0x6100..=0x61FF => Some(Self::InternalSoftware),
            0x6200..=0x62FF => Some(Self::UserSoftware),
            0x6300..=0x63FF => Some(Self::DataSet),
            0x8111..=0x811F => Some(Self::Communication),
            0x8121..=0x812F => Some(Self::Communication),
            0x8131..=0x813F => Some(Self::Communication),
            0x8141..=0x814F => Some(Self::Communication),
            0x8151..=0x81FF => Some(Self::Communication),
            0x8211..=0x821F => Some(Self::ProtocolError),
            0x8221..=0x822F => Some(Self::ProtocolError),
            0x8231..=0x823F => Some(Self::ProtocolError),
            0x8241..=0x824F => Some(Self::ProtocolError),
            0x8251..=0x82FF => Some(Self::ProtocolError),
            0x2000..=0x20FF => Some(Self::Current),
            0x3000..=0x30FF => Some(Self::Voltage),
            0x4000..=0x40FF => Some(Self::Temperature),
            0x5000..=0x50FF => Some(Self::DeviceHardware),
            0x6000..=0x60FF => Some(Self::DeviceSoftware),
            0x7000..=0x70FF => Some(Self::AdditionalModules),
            0x8000..=0x80FF => Some(Self::Monitoring),
            0x8200..=0x820F => Some(Self::ProtocolError),
            0x9000..=0x90FF => Some(Self::ExternalError),
            0xF000..=0xF0FF => Some(Self::AdditionalFunctions),
            0xFF00..=0xFFFF => Some(Self::DeviceSpecific),
            0x0000..=0x00FF => Some(Self::ErrorResetOrNoError),
            0x1000..=0x10FF => Some(Self::GenericError),
            _ => None,
        }
        .ok_or_else(|| CanOpenError::ParseError(format!("bad error code: {}", code)))
    }
    pub fn encode(&self) -> u16 {
        match self {
            Self::ErrorResetOrNoError => 0x0000,
            Self::GenericError => 0x1000,
            Self::Current => 0x2000,
            Self::CurrentInputSide => 0x2100,
            Self::CurrentInsideDevice => 0x2200,
            Self::CurrentOutputSide => 0x2300,
            Self::Voltage => 0x3000,
            Self::MainsVoltage => 0x3100,
            Self::VoltageInsideDevice => 0x3200,
            Self::OutputVoltage => 0x3300,
            Self::Temperature => 0x4000,
            Self::AmbientTemperature => 0x4100,
            Self::DeviceTemperature => 0x4200,
            Self::DeviceHardware => 0x5000,
            Self::DeviceSoftware => 0x6000,
            Self::InternalSoftware => 0x6100,
            Self::UserSoftware => 0x6200,
            Self::DataSet => 0x6300,
            Self::AdditionalModules => 0x7000,
            Self::Monitoring => 0x8000,
            Self::Communication => 0x8100,
            Self::CommunicationCanOverrun => 0x8110,
            Self::CommunicationErrorPassiveMode => 0x8120,
            Self::CommunicationLifeGuardError => 0x8130,
            Self::CommunicationRecoveredBusOff => 0x8140,
            Self::CommunicationCanIdCollision => 0x8150,
            Self::ProtocolError => 0x8200,
            Self::ProtocolErrorPdoLength => 0x8210,
            Self::ProtocolErrorPdoLengthExceeded => 0x8220,
            Self::ProtocolErrorDamMpdo => 0x8230,
            Self::ProtocolErrorUnexpectedSyncLength => 0x8240,
            Self::ProtocolErrorRpdoTimeout => 0x8250,
            Self::ExternalError => 0x9000,
            Self::AdditionalFunctions => 0xF000,
            Self::DeviceSpecific => 0xFF00,
        }
    }
}

#[derive(Clone, Debug)]
pub enum EmergencyErrorRegister {
    GenericError,
    Current,
    Voltage,
    Temperature,
    CommunicationError,
    DeviceProfileSpecific,
    Reserved,
    ManufacturerSpecific,
}

impl EmergencyErrorRegister {
    pub fn decode(code: u8) -> Vec<Self> {
        let mut errors = Vec::new();
        if code & 0x01 != 0 {
            errors.push(Self::GenericError);
        }
        if code & 0x02 != 0 {
            errors.push(Self::Current);
        }
        if code & 0x04 != 0 {
            errors.push(Self::Voltage);
        }
        if code & 0x08 != 0 {
            errors.push(Self::Temperature);
        }
        if code & 0x10 != 0 {
            errors.push(Self::CommunicationError);
        }
        if code & 0x20 != 0 {
            errors.push(Self::DeviceProfileSpecific);
        }
        if code & 0x40 != 0 {
            errors.push(Self::Reserved);
        }
        if code & 0x80 != 0 {
            errors.push(Self::ManufacturerSpecific);
        }
        errors
    }

    pub fn encode(errors: &[EmergencyErrorRegister]) -> u8 {
        let mut code = 0;
        for error in errors {
            code |= match error {
                Self::GenericError => 0x01,
                Self::Current => 0x02,
                Self::Voltage => 0x04,
                Self::Temperature => 0x08,
                Self::CommunicationError => 0x10,
                Self::DeviceProfileSpecific => 0x20,
                Self::Reserved => 0x40,
                Self::ManufacturerSpecific => 0x80,
            };
        }
        code
    }
}
