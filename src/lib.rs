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

    fn validate_struct_refs(&self, wanted_by: &str, segments: &Vec<PacketSegment>) {
        //TODO: can metadata contain a struct?
        for segment in segments {
            match segment {
                PacketSegment::Struct { name: field_name, struct_name } => {
                    assert!(self.structs.contains_key(struct_name), "Undefined struct \"{struct_name}\" referenced by ({wanted_by}->{field_name})");
                    let rs = self.structs.get(struct_name).expect("Struct was just validated but in any case, failed to dereference struct \"{struct_name}\" mentioned by \"{payload_name}\"");

                    self.validate_struct_refs(&format!("{wanted_by}->[Struct {struct_name}]{field_name}"), &rs.fields);
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
                    assert!(self.structs.contains_key(item_struct), "Undefined struct \"{item_struct}\" referenced by ({wanted_by}->{field_name})");
                    let rs = self.structs.get(item_struct).expect("Struct was just validated but in any case, failed to dereference struct \"{struct_name}\" mentioned by \"{payload_name}\"");
                    self.validate_struct_refs(&format!("{wanted_by}->[Array of {item_struct}]{field_name}"), &rs.fields);
                }
                // if we ever add a variant in unsized that references a struct, the
                // appropriate validation should be added here
                PacketSegment::Unsized { datatype: UnsizedDataType::StringUTF8 | UnsizedDataType::Raw, .. } => ()
            }
        }
    }

    pub fn validate_no_unsized_unterminated_rx(&self, wanted_by: &str, segments: &Vec<PacketSegment>) {
        for segment in segments {
            match segment {
                PacketSegment::Unsized { name, datatype, termination, description: _ } => {
                    assert!(!termination.is_none(), "Unterminated unsized reads not possibe. One was found in {wanted_by}->{name}");

                    match datatype {
                        UnsizedDataType::Raw => (),
                        UnsizedDataType::StringUTF8 => (),
                        UnsizedDataType::Array { item_struct } => {
                            self.validate_no_unsized_unterminated_rx(&format!("{wanted_by}->[Array of {item_struct}]{name}"), segments)
                        }
                        // (TODO: enum-structs when support is added, recurse through an enum. Also
                        // be aware of case for untagged enum structs. (i.e. untagged unions)
                    }
                },
                PacketSegment::Struct { name, struct_name } => {
                    self.validate_no_unsized_unterminated_rx(&format!("{wanted_by}->[Struct {struct_name}]{name}"), segments)
                },
                PacketSegment::Sized { .. } => ()
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
        // return value references fields that exist
        // make sure references to metadata exist in all packets
        // metadata cannot contain Const, since it's basically already Constant. Use PacketFormatElement::Const instead
        // metadata literals are correct and compatible in each packet

        // no self-references or reference cycles possible... none for structs etc. 

        // While we currently don't support Sized references to structs, any sized references to
        // structs must reference sized structs
        
        // RULE: make sure that no RX packets have Unterminated Unsized data types, either directly or
        // through a struct 
        for (payload_name, payload) in &self.payloads.rx {
            self.validate_no_unsized_unterminated_rx(payload_name, &payload.segments);
        }

        // RULE: All refs to payloads in transactions exist
        for (wanted_by_transaction, transaction) in &self.transactions {
            for action in &transaction.actions {
                 match action {
                    Action::Tx { payload } => {
                        assert!(self.payloads.tx.contains_key(payload), "Undefined TX payload \"{payload}\" referenced by transaction \"{wanted_by_transaction}\"");
                    },
                    Action::Rx { payload } => {
                        assert!(self.payloads.rx.contains_key(payload), "Undefined RX payload \"{payload}\" referenced by transaction \"{wanted_by_transaction}\"");
                    },
                    Action::Sleep { .. } => (),
                    Action::Flush { .. } => ()
                }
            }
        }

        // RULE: all references to structs must be valid
        for (payload_name, payload) in self.payloads.tx {
            self.validate_struct_refs(&payload_name, &payload.segments);
        }
        for (payload_name, payload) in self.payloads.rx {
            self.validate_struct_refs(&payload_name, &payload.segments);
        }


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
