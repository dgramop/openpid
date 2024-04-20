#[macro_use]
extern crate derive_more;

pub mod config;

pub mod prelude {
    pub use crate::config::*;
    pub use crate::Codegen;
}

use std::{collections::BTreeMap, fmt::Display};

use convert_case::Casing;
use prelude::*;
pub use config::OpenPID;

#[derive(Debug)]
pub enum CodegenError {
    IOError(std::io::Error),
    NoStruct { wanted_by_payload: String, wanted_by_field: String, struct_name: String}
}

impl Display for CodegenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CodegenError::NoStruct { wanted_by_payload, wanted_by_field, struct_name } =>{
                write!(f, "Couldn't find struct named {struct_name} referenced by {wanted_by_payload}->{wanted_by_field}")
            }
            CodegenError::IOError(e) => {
                write!(f, "Input/Output Error: {:?}", e)
            }
        }
    }
}

impl From<std::io::Error> for Box<CodegenError> {
    fn from(value: std::io::Error) -> Self {
        Box::new(CodegenError::IOError(value))
    }
}

pub trait Codegen {
    fn codegen(&mut self) -> Result<(), CodegenError>;
}

impl OpenPID {
    pub fn from_str(a: &str) -> Result<Self, toml::de::Error> {
        toml::from_str(a)
    }

    fn validate_struct_refs(&self, payloads: &BTreeMap<String, Payload>) {
        //TODO: can metadata contain a struct?
        for (payload_name, payload) in payloads {
            for segment in &payload.segments {
                match segment {
                    PacketSegment::Struct { name: field_name, struct_name } => {
                        assert!(self.structs.contains_key(struct_name), "Undefined struct \"{struct_name}\" referenced by ({payload_name}->{field_name})")
                    },

                    // if we ever add a variant in sized that references a struct, for
                    // example a known-length list, this won't compile and you should
                    // add the appropriate validation here
                    PacketSegment::Sized {
                        datatype: SizedDataType::Integer { .. } 
                        | SizedDataType::FloatIEEE { .. }
                        | SizedDataType::Raw
                        | SizedDataType::Const { .. }
                        | SizedDataType::StringUTF8, 
                        ..
                        } => (), 
                    PacketSegment::Unsized { name: field_name, datatype: UnsizedDataType::Array { item_struct }, ..} => {
                        assert!(self.structs.contains_key(item_struct), "Undefined struct \"{item_struct}\" referenced by ({payload_name}->{field_name})")
                    }
                    // if we ever add a variant in unsized that references a struct, the
                    // appropriate validation should be added here
                    PacketSegment::Unsized { datatype: UnsizedDataType::StringUTF8 | UnsizedDataType::Raw, .. } => ()
                }
            }
        }
    }

    // This function is and should be optimized for readability and correctness over performance.
    // In particular, it should be easy to ascertain that this function enforces all the underlying
    // validation rules. For example, it is acceptable to iterate over the same data separately when 
    // enforcing different rules
    pub fn validate(&self) {
        // All count_in_packets for dynamic-length data refer to a field that exists
        // make sure that all packet refs exist
        // make sure that all refs to payloads in transactions exist
        // make sure that no RX packets have Unterminated Unsized data types, either directly or
        // through a struct
        // make sure rx packet format does not have an Unterminated Unsized data type
        // return value references fields that exist
        // make sure references to metadata exist in all packets
        // metadata cannot contain Const, since it's basically already Constant. Use PacketFormatElement::Const instead
        // metadata literals are correct and compatible in each packet

        // RULE: all references to structs must be valid
        self.validate_struct_refs(&self.payloads.tx);
        self.validate_struct_refs(&self.payloads.rx);


        // RULE:
        // all names, except for the device name should be in lower-snake case. Codegen will take care of making names into
        // camelcase or snakecase depending on what's idiomatic for that language
        for name in self.structs.keys() {
            assert!(name.is_case(convert_case::Case::Snake), "Struct \"{name}\" is not snake case");
        }
        for name in self.payloads.tx.keys() {
            assert!(name.is_case(convert_case::Case::Snake), "TX Payload \"{name}\" is not snake case");
        }
        for name in self.payloads.rx.keys() {
            assert!(name.is_case(convert_case::Case::Snake), "RX Payload \"{name}\" is not snake case");
        }
        for name in self.transactions.keys() {
            assert!(name.is_case(convert_case::Case::Snake), "Transaction \"{name}\" is not in snake case");
        }
    }
}

impl Payload {
    /// Estimates a payload's size, not including headers etc.
    pub fn get_size() -> u32 {
        todo!()
    }
}
