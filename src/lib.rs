#[macro_use]
extern crate derive_more;

pub mod config;

pub mod prelude {
    pub use crate::config::*;
    pub use crate::Codegen;
}

use prelude::*;
pub use config::OpenPID;

#[derive(Debug, Display)]
pub enum CodegenError {
    IOError(std::io::Error)
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

    pub fn validate(&self) {
        // All count_in_packets refer to a field that exists
        // make sure that all struct refs exist
        // make sure that all packet refs exist
        // make sure that no RX packets have Unterminated Unsized data types, either directly or
        // through a struct
        // make sure rx packet format does not have an Unterminated Unsized data type
        // return value references fields that exist
        // make sure references to metadata exist in all packets
        // metadata cannot contain Const, since it's basically already Constant. Use PacketFormatElement::Const instead
        // metadata literals are correct and compatible in each packet
    }
}

impl Payload {
    /// Estimates a payload's size, not including headers etc.
    pub fn get_size() -> u32 {
        todo!()
    }
}
