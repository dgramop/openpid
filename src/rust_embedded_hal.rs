use indoc::formatdoc;

use crate::prelude::*;
use std::fs;

struct RustEmbeddedHal<'a> {
    pid: &'a OpenPID,
    target: std::path::PathBuf
}

impl<'a> RustEmbeddedHal<'a> {
    fn new(pid: &'a OpenPID, target: std::path::PathBuf) -> Self {
        Self {
            pid,
            target
        }
    }
    
    /// Initializes Cargo Package at the given path
    fn cargo_init(&self) -> Result<(), std::io::Error> {
        fs::create_dir_all(&self.target)?;
        fs::create_dir_all(&self.target.join("src"))?;
        fs::write(self.target.join(".gitignore"), formatdoc! {"
        target/
        Cargo.lock
        **/*.rs.bk
        debug/
        *.pdb
        "})?;

        let version = match &self.pid.doc_version {
            Some(version) => {
                version
            },
            None => {
                println!("No document version provided. This may result in problems with cargo publish later on. This behavior is deprecated and will abort codegen in the future. Defaulting codegen'd crate version 0.1.0");
                "0.1.0"
            }
        };

        fs::write(&self.target.join("Cargo.toml"), formatdoc!("
        [package]
        name = \"{name}\"
        version = \"{version}\"
        edition = \"2021\"
        authors = [\"OpenPID Codegen\"]
        description = \"{desc}\"
        categories = [\"embedded\", \"no-std\", \"parser-implementations\", \"hardware-support\"]
        keywords = [\"driver\", \"openpid\"]

        [dependencies]
        embedded_hal = \"1\"
        ", 
        name = self.pid.device_info.name,
        version = version,
        desc = self.pid.device_info.description
        ))?;

        let src_dir = self.target.join("src");

        fs::write(src_dir.join("lib.rs"), formatdoc!("
        extern crate embedded_hal;

        pub enum SizedDataType {{
            /// Integral number
            Integer {{ endianness: Endianness, signing: Signing }},

            /// An IEEE float
            FloatIEEE {{ endianness: Endianness }},

            /// Raw array of bytes
            Raw,
            // TODO enum and 
            // enum variant
            // TODO bool

            /// Represents a UTF8 string
            StringUTF8,

            /// Hardcoded data
            Const {{ data: Vec<u8> }}
        }}

        pub enum UnsizedDataType {{
            /// Several Repetitions of a given type
            Array {{
                /// This is basically a simplified sub-packet. 
                item_struct: String,
            }},
            
            /// Represents a UTF8 string
            StringUTF8,

            /// Raw array of bytes
            Raw,

        }}

        // TODO: just pull types from UnsizedDataType/SizedDataType. Maybe we need another crate
        // called openpid_types
        pub enum EmbeddedType {{
            Sized(SizedDataType),
            Unsized(UnsizedDataType)
        }}

        struct BitStream {{
            /// Underlying byte stream
            stream: todo!(),

            /// Stores bits if a non-byte-aligned read/write occurs, for future reading/writing
            leftover: u8
        }}

        /// Reads the given type from the bit stream
        pub trait<Underlying, const ET: EmbeddedType> Get<T> {{
            fn get(stream: BitStream) -> T;
        }}

        /// Writes the given type to the bit stream
        pub trait<Underlying, const ET: EmbeddedType> Put<Underlying, const ET: EmbeddedType> {{
            /// Returns the number of bits written
            fn put(steam: BitStream) -> usize;
        }}

        impl Get<EmbeddedType::Sized(SizedDataType::Integer)>

        "))?;

        todo!()
    }

    /// Produces a header for lib.rs
    fn write_lib(&self) {

    }
}

impl<'a> Codegen for RustEmbeddedHal<'a> {
    fn codegen(&mut self) -> Result<(), crate::CodegenError> {
        self.cargo_init();

        Ok(())
    }
}
