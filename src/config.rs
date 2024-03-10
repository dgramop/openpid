use std::{collections::BTreeMap, error::Error};
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize, Debug)]
#[serde(untagged)]
pub enum OneOrMany<T> {
    One(T),
    Many(Vec<T>),
}

impl<T> OneOrMany<T> {
    pub fn as_many(self) -> Vec<T> {
        match self {
            OneOrMany::One(one) => vec![one],
            OneOrMany::Many(vec) => vec
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub enum BitsOrBytes {
    Bits,
    Bytes
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ReusableStruct {
    /// Name of this struct, used in codegen and to reference this struct from other fields
    pub name: String,
    pub fields: Vec<PacketFormatElement>,
    //TODO: privacy?
}

#[derive(Serialize, Deserialize, Default, Debug)]
pub enum Endianness {
    /// Most significant bit shows up first (at a lower memory address). If we visualize memory
    /// addresses as increasing from left to right, the most significant bit would be on the left,
    /// closest to how most of the world represents numbers. By digit, we refer to a byte.
    BigEndian,

    /// More common. Least significant bit shows up first (at a lower memory address). If we visualize memory
    /// addresses as increasing from left to right, the digits would be backwards. By digit, we
    /// refer to a byte
    #[default]
    LittleEndian
}

#[derive(Serialize, Deserialize, Default, Debug)]
pub enum Signing {
    /// Uses the first bit to flag negative numbers. 
    OnesComplement,

    /// Uses the two's complement rules to handle negative numbers. This is more common and the
    /// default on most computers
    TwosComplement,

    #[default]
    Unsigned
}

/// Strategy for terminating an array. How should we know when to stop reading from the device?
#[derive(Serialize, Deserialize, Debug)]
#[serde(untagged)] //make untagged?
pub enum Terminator {
    /// Reads/Writes this many elements
    CountFixed { count: u32 },

    /// Uses the previously-read field name. Must be a field name referenced as part of the same
    /// packet. Inserts the Count at the given field name
    CountInPacket { field_name: String },

    /// Stops when it finds the given pattern, writes the pattern at the end
    Sequence { sequence: Vec<u8> },

    //TODO: we should describe transactions that read/write multiple packets, in case the size is
    //supplied in another packet
    // CountInTransaction {
    // packet_name: String
    // field_name: String
    // }

}

// variant names chosen with their higheset-level datatype descriptor first, to make them easier to find
// data type structs are kept separate to allow for easier optimization of generated code, so that
// packets that are completely sized can be read from stream in a single shot
/// Represents a particular piece of data's type. In the literal sense, describes its
/// interpretation. The actual length of the data is specified in bits elsewhere
#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "type")]
pub enum SizedDataType {
    //TODO: string and array are unsized. Maybe we should embed size into this enum

    /// Integral number
    Integer { endianness: Endianness, signing: Signing },

    /// An IEEE float
    FloatIEEE { endianness: Endianness },

    /// Raw array of bytes
    Raw,
    // TODO enum and 
    // enum variant

    /// Represents a UTF8 string
    StringUTF8,

    /// Hardcoded data
    Const { data: Vec<u8> }
}

#[derive(Serialize, Deserialize, Debug)]
pub enum UnsizedDataType {
    /// Several Repetitions of a given type
    Array {
        /// This is basically a simplified sub-packet. 
        item_struct: String,
    },
    
    /// Represents a UTF8 string
    StringUTF8,

    /// Raw array of bytes
    Raw,

    // TODO enum_struct in unsized, for cases where other fields are tied to the
}

#[derive(Serialize, Deserialize, Debug)]
pub enum Crc {
    // there are tons of CRC implementations. TODO: list as many as possible here, including
    // infamous CRC16 XMODEM
    Crc32,
    Crc16XModem,
}

//in variants that are integer sizes, leave out signing flag beacuse ones&twos complement repr's are the same
//for positive numbers
#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "type")]
pub enum PacketFormatElement {
    /// The Total Size of the packet, including the payload, all headers (Crc etc.) 
    SizeTotal {
        size_bits: u32, 
        express_as: BitsOrBytes
    },

    /// Size of the Payload only
    SizeOfPayload {
        size_bits: u32,
        express_as: BitsOrBytes
    },

    //TODO: packetID here instead of payload

    /// Size of all the elements listed in `elements`
    SizeOfElements {
        size_bits: u32,
        express_as: BitsOrBytes,
        elements: Vec<PacketFormatElement>
    },

    /// The payload of the actual packet
    Payload,

    /// Reference metadata from the payload for use in a header/footer, for example a packet ID
    Metadata { 
        key: String
    },

    /// Crc/hash strategy
    Crc { algorithm: Crc },

    /// A fixed value/flag to include in every packet
    Const { data: Vec<u8> }
}

type PacketFormat = Vec<PacketFormatElement>;

//TODO: multiple send/recieve formats? What if a packet is both send and recieve?
#[derive(Serialize, Deserialize, Debug)]
pub struct AllPacketFormats {
    pub tx: PacketFormat, 
    pub rx: PacketFormat
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(untagged)]
pub enum PacketSegment {
    Sized {
        name: String,
        
        bits: u32,

        #[serde(rename = "type")]
        datatype: SizedDataType
    },
    Unsized {
        name: String,

        #[serde(rename = "type")]
        datatype: UnsizedDataType,

        /// If None, the packet can only be TX'd! (TODO: codegen-time check this)
        /// In this case, whatever the libary developer writes will be sent, and the size of what
        /// is sent will not be communicated in any way to the device, except through the overall
        /// packet/payload size, if included in the packet format
        termination: Option<Terminator>
    },
    Struct { name: String, struct_name: String }
}

impl PacketSegment {
    pub fn get_name(&self) -> &str {
        match self {
            Self::Sized { name, .. } => name,
            Self::Unsized { name, .. } => name,
            Self::Struct { name, ..} => name
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Payload {
    /// Data inside this packet, in segments
    pub segments: Vec<PacketSegment>,

    /// Metadata that can be referenced by the PacketFormat, for example a packet ID
    #[serde(flatten)]
    pub metadata: BTreeMap<String, OneOrMany<PacketSegment>>,
    
    /// Optional description documentation
    pub description: String
}

#[derive(Serialize, Deserialize, Debug)]
pub struct AllPayloads {
    /// Packet formats that are sendable
    pub tx: BTreeMap<String, Payload>,

    /// Packet formats that are recievable
    pub rx: BTreeMap<String, Payload>,
}

/// An action that can be taken during a transaction
#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "type")]
pub enum Action {
    /// Send a packet with the given name
    Tx { payload: String },

    /// Receive a packet with the given name
    Rx { payload: String },

    /// Sleep for this many milliseconds
    Sleep { milliseconds: u32 },

    /// Flush/empty out the buffer, discarding all data
    Flush
}

//TODO: a way to sleep + flush buffer
/// Represents a grouping of packets, send or receive, to be performed in order
#[derive(Serialize, Deserialize, Debug)]
pub struct Transaction {
    /// An ordered list of actions to take during a transaction, like sending or recieving a
    /// packet, or like sleeping or flushing the buffer
    pub actions: Vec<Action>,

    /// List of field names to return (<packet>.<field>)
    pub returns: Vec<String>,

    /// Describes what this Transaction does
    pub description: String
}

#[derive(Serialize, Deserialize, Debug)]
pub struct DeviceInfo {
    pub name: String,
    pub description: String
}

//TODO: for interrupt-based systems, it would be nice to automatically parse the packet data
//type/packetID based on the struct, and maybe even set a default transaction for that case.
//TODO: this document is currently very UART-binary/streaming-focused. Maybe we should come up with
//a similar format that uses some of the same structs for I2C etc. For i2c, we would re-use
//transaction, but registers are basically fixed-size packets. 
//TODO change ids so that they have types wrapped around them
//TODO: config for I2C, SPI, UART (default baud etc.)
#[derive(Serialize, Deserialize, Debug)]
pub struct OpenPID {
    /// Information about the device
    pub device_info: DeviceInfo,

    /// Version of OpenICD to use
    pub openpid_version: Option<String>,

    /// This document's version
    pub doc_version: Option<String>,

    /// Each packet format specifies what part of the packet goes where, sequence
    /// numbers, length fields, and other formatting choices that describe the entire packet. The
    /// lowest level description of your interface  
    pub packet_formats: AllPacketFormats,

    /// Referenced by packets, describes re-usable packet contents that may be sent or recieved
    /// to/from the device 
    pub structs: BTreeMap<String, ReusableStruct>,

    /// Describes the actual contents of the packets themselves, the next highest level description
    /// of your interface
    pub payloads: AllPayloads,

    /// The highest level of your interface representable by OpenICD. If you want higher-level
    /// SDKs, you can wrap the codegen to make fancier stuff. The codegen will give you an
    /// excellent starting point so you can focus on creating value
    pub transactions: BTreeMap<String, Transaction>
    
    //TODO Higher level access that describes the state machines present in the device
    //this is for builder-style workflows, stuff that might need logic like a switch statement. For
    //example, a response that tells you what the type of the next packet is going to be, or that
    //tells us what our next request should be
    //state_machines: todo!(),
    //TODO: not all platforms should have to support the state machine interface, to make it easier
    //for people to "just release" platforms and get at least minimal access to sensors
    //TODO: higher level config for responses
}

pub struct Transition {

    //TODO: language-agnostic boolean expression
    /// Decides if this transition is taken
    given: String,

    //TODO: how do we arguments/symbols?
    //probably take all the arguments/symbols from the transaction used to get to this state, and
    //use them as arguments
    //
    //We might be pushing the limits of TOML here, trying to represent such a state machine
    //Could have the user explicitly list what data to retain in each state/make contracts for what
    //data we need. And then specify variable renamings or something
    /// Transaction to execute if this transition is taken
    transaction: Transaction,

    /// Destination state
    to: String,
}

pub struct State {
    next: Vec<Transition>
}

type StateMachine = BTreeMap<String, State>;

fn stub() -> Result<(), Box<dyn Error>> {
    let structs = BTreeMap::<String, ReusableStruct>::new();
    let transactions = BTreeMap::<String, Transaction>::new();

    let tx_payloads = BTreeMap::<String, Payload>::new();
    let rx_payloads = BTreeMap::<String, Payload>::new();
    println!("{}",toml::to_string_pretty(&OpenPID {
        device_info: DeviceInfo { name: "Your Device".to_owned(), description: "Brief description".to_owned() },
        openpid_version: None,
        doc_version: None,
        packet_formats: AllPacketFormats { 
            tx: PacketFormat::new(),
            rx: PacketFormat::new()
        },
        structs,
        payloads: AllPayloads { tx: tx_payloads, rx: rx_payloads },
        transactions,
    })?);
    Ok(())
}
