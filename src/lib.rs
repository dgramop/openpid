#[macro_use]
extern crate derive_more;

pub mod config;

pub mod prelude {
    pub use crate::config::*;
}

use indoc::formatdoc;
use prelude::*;

#[derive(Debug, Display)]
pub enum CodegenError {
    IOError(std::io::Error)
}

impl From<std::io::Error> for Box<CodegenError> {
    fn from(value: std::io::Error) -> Self {
        Box::new(CodegenError::IOError(value))
    }
}

const INDT: &'static str = "  ";

impl OpenPID {
    fn segment_to_c_vars(segment: &PacketSegment) -> Vec<String> {
        match segment {
            PacketSegment::Sized { name, bits, datatype } => {
                let type_name = match datatype {
                    SizedDataType::StringUTF8 => "char*".to_owned(),

                    // TODO: bitshifting for non-multiples-of-8-bit-types
                    SizedDataType::Integer { endianness: _, signing } => format!("{sign}int{bits}_t", sign = match signing {
                        Signing::OnesComplement | Signing::TwosComplement => "",
                        Signing::Unsigned => "u"
                    }),
                    SizedDataType::Raw => {
                        "byte*".to_owned()
                    },
                    SizedDataType::Const { data } => {
                        //should be skipped
                        return vec![];
                    },
                    SizedDataType::FloatIEEE { endianness } => {
                        match bits {
                            32 => "float".to_owned(),
                            64 => "double".to_owned(),
                            o => todo!("Unknown IEEE {}bit float. Only supports 32 and 64 bit floats. Consider taking the data as Raw and using wrapper code/patching the codegen to implement your custom floating point type.  Needs Error handling", o)
                        }
                    }
                };

                vec![ format!("{type_name} {name}") ]

            },
            PacketSegment::Unsized { name, datatype, termination: _ } => {
                match datatype {
                    // no need for multiple arguments since we will expect null terminated strings
                    UnsizedDataType::StringUTF8 => vec![format!("char* {name}")],
                    UnsizedDataType::Raw => {
                        vec![format!("byte* {name}"), format!("size_t {name}_length")]
                    },
                    UnsizedDataType::Array { item_struct } => {
                        vec![format!("struct {item_struct}* {name}"), format!("size_t {name}_length")]
                    },
                }
            },
            PacketSegment::Struct { name , struct_name} => unimplemented!("No struct support yet")
        }
    }

    pub fn c_emit_tx_function(&self, name: &str, payload: &Payload) -> Result<String, Box<CodegenError>> {
        let description = &payload.description;
        let args = payload.segments.iter()
            .map(Self::segment_to_c_vars            )
            .flatten()
            .collect::<Vec<_>>()
            .join(", ");

        let mut writes = String::new();

        for segment in &payload.segments {
            match segment {
                PacketSegment::Sized { name, bits, datatype } => {
                    match datatype {
                        SizedDataType::Integer { endianness, signing } => {
                            // TODO set endianness. If signing is one's complement, change accordingly
                            // TODO: bitshifting for non-multiples-of-8-bit-types
                            writes.push_str(&format!("{INDT}device->write((byte*) &{name}, {bits});\n"));
                        },
                        SizedDataType::Raw => {
                            // just write the argument straight up
                            writes.push_str(&format!("{INDT}device->write({name}, {bits});\n"));
                        },
                        SizedDataType::StringUTF8 => {
                            writes.push_str(&format!("{INDT}device->write((byte*) {name}, {bits});\n"));
                        },
                        SizedDataType::FloatIEEE { endianness } => {
                            //TODO set endianness
                            writes.push_str(&format!("{INDT}device->write((byte*) &{name}, {bits});\n"));
                        },
                        SizedDataType::Const { data } => {
                            let data_byte_length = data.len();
                            let data_array = data.iter().map(|e| e.to_string()).collect::<Vec<_>>().join(", ");
                            writes.push_str(&formatdoc!("
                            {INDT}byte {name}[{data_byte_length}] = [{data_array}];
                            {INDT}device->write({name}, {bits});
                            "));
                        }
                    }
                },
                PacketSegment::Unsized { name, datatype, termination } => {
                    // write the actual data
                    match datatype {
                        UnsizedDataType::Raw | UnsizedDataType::Array { .. } => {
                            writes.push_str(&format!("{INDT}device->write({name}, {name}_length);\n"))
                        },
                        UnsizedDataType::StringUTF8 => {
                            writes.push_str(&format!("{INDT}device->write({name}, strlen({name})*8);\n"))
                        }
                    }

                    // perform the termination (which may in some cases, involve prepending
                    // variables that are inserted earlier into the program
                    match (termination, datatype) {
                        // when writing, this is effectively unterminated
                        (Some(Terminator::CountFixed { count: _ }), _) => (),
                        (Some(Terminator::CountInPacket { field_name }), dt) => {
                            //TODO: do not collect field names as arguments for TX if they are referred to by
                            //CountInPacket, since we are instead collecting _length (or in the
                            //case of strings, we are just using strlen). Of course, even though we
                            //don't collect them in arguments, we should still write them. Their
                            //computation/assignment should be done before they are written! This
                            //is why we prepend to the writes string

                            // while this simple re-assignment is basically a no-op, c optimizer
                            // will eliminate performance hit
                            let target_segment = payload.segments.iter().find(|i| i.get_name() == field_name).expect(&format!("Ref not found for {}", field_name));

                            assert!(match target_segment {
                                PacketSegment::Sized { .. } => true,
                                _ => false
                            }, "The variable ({seg_name}) into which the count for {name} is being inserted must be sized, ideally a sized integer", seg_name = segment.get_name());

                            let vars = Self::segment_to_c_vars(target_segment);

                            // This must be some kind of integer type, cannot be unsized
                            assert_eq!(vars.len(), 1, "Since the segment is sized and not a constant, there should be one variable emitted from it");

                            let var = &vars[0];

                            match dt {
                                UnsizedDataType::Raw | UnsizedDataType::Array { .. } => {
                                    writes = format!("{INDT}{var} = {name}_length;\n{writes}");
                                },
                                UnsizedDataType::StringUTF8 => {
                                    writes = format!("{INDT}{var} = strlen({name})*8;\n{writes}");
                                }
                            }
                        },
                        (Some(Terminator::Sequence { sequence }), _)=> {
                            // the same as writing raw
                            let data_array = sequence.iter().map(|e| e.to_string()).collect::<Vec<_>>().join(", ");
                            writes.push_str(&formatdoc!("
                            byte* {name}_terminator = {{ {data_array} }};
                                    device->write({name}_terminator);
                                    "))
                        },
                        // Unterminated. This is OK for transmit 
                        (None, _) => ()
                    };
                },
                PacketSegment::Struct { name , struct_name} => {
                    todo!()
                }
            }
        }

        Ok(formatdoc!("
        \n\n
        // {description}
        void {name}(struct Device* device, {args}) {{
        {writes}
        }}"))
    }
    
    pub fn c_emit_rx_function(&self, name: &str, payload: &Payload) -> Result<String, Box<CodegenError>> {
        let description = &payload.description;
        let return_struct_filler = payload.segments.iter()
            .map(Self::segment_to_c_vars            )
            .flatten()
            .map(|t| format!("{INDT}{t};"))
            .collect::<Vec<_>>()
            .join("\n");

        let return_struct_name = format!("struct {name}");

        let return_struct = formatdoc!("
        {return_struct_name} {{ 
        {return_struct_filler} 
        }};");

        Ok(formatdoc!("
        \n\n{return_struct}

        // {description}
        {return_struct_name} {name}(struct Device* device) {{
            
        }}"))
    }

    /// Generates C program code to the given destination directory
    pub fn codegen_linux_c(&self, destination: std::path::PathBuf) -> Result<(), Box<CodegenError>> {
        println!("{:?}",destination.exists());
        let file = destination.join("lib.c");
        let mut contents = formatdoc!("
            #include <stdio>
            #include <stdlib>

            struct Device {{
            {INDT}// Writes data with length to the device, returning bytes written, or a negative
            {INDT}// number for an error.
            {INDT}int (*write)(byte* data, size_t length_bits),

            {INDT}// Reads data with max length from the device, returning bytes read or a negative
            {INDT}// number for an error
            {INDT}int (*read)(byte* data, size_t length_bits) read,
            }}");

        for (name, payload) in self.payloads.tx.iter() {
            contents.push_str(&self.c_emit_tx_function(name, payload)?)
        }

        for (name, payload) in self.payloads.rx.iter() {
            contents.push_str(&self.c_emit_rx_function(name, payload)?)
        }

        println!("{contents}");

        std::fs::write(file, contents)?;

        Ok(())
    }

    fn validate(&self) {
        // All count_in_packets refer to a field that exists
        // make sure that all struct refs exist
        // make sure that all packet refs exist
        // make sure that no RX packets have Unterminated Unsized data types, either directly or
        // through a struct
        // make sure rx packet format does not have an Unterminated Unsized data type
        // return value references fields that exist
    }
}
