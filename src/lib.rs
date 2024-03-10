#[macro_use]
extern crate derive_more;
extern crate rand;

pub mod config;

pub mod prelude {
    pub use crate::config::*;
}

use indoc::formatdoc;
use prelude::*;
use rand::Rng;

#[derive(Debug, Display)]
pub enum CodegenError {
    IOError(std::io::Error)
}

impl From<std::io::Error> for Box<CodegenError> {
    fn from(value: std::io::Error) -> Self {
        Box::new(CodegenError::IOError(value))
    }
}

/// Units of indentation
const INDT: &'static str = "  ";

impl PacketSegment {
    fn to_c_vars(&self) -> Vec<String> {
        match self {
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
                    //should be skipped, since we will hardcode the contents
                    SizedDataType::Const { .. } => {
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
}

impl OpenPID {
    fn emit_struct(&self, struct_: &ReusableStruct) -> String {
        let name = &struct_.name;
        let fields = struct_.fields.iter()
            .map(|s| s.to_c_vars())
            .flatten()
            .map(|var| format!("{INDT}{var};"))
            .collect::<Vec<_>>()
            .join("\n");

        formatdoc!("
        struct {name} {{
        {fields}
        }};
        ")
    }


    /// Setup and calls to device->write() for each payload segment. May be used for writing
    /// metadata, or other segment data
    fn segment_writes(&self, segments: &Vec<PacketSegment>) -> String {
        let mut writes = String::new();

        for segment in segments {
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
                            {INDT}byte {name}[{data_byte_length}] = {{ {data_array} }};
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
                            let target_segment = segments.iter().find(|i| i.get_name() == field_name).expect(&format!("Ref not found for {}", field_name));

                            assert!(match target_segment {
                                PacketSegment::Sized { .. } => true,
                                _ => false
                            }, "The variable ({seg_name}) into which the count for {name} is being inserted must be sized, ideally a sized integer", seg_name = segment.get_name());

                            let vars = target_segment.to_c_vars();

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

        writes
    }

    /// Emits a transmit packet function for a given payload
    pub fn c_emit_tx_function(&self, name: &str, payload: &Payload) -> Result<String, Box<CodegenError>> {
        let description = &payload.description;
        let args = payload.segments.iter()
            .map(|s| s.to_c_vars())
            .flatten()
            .collect::<Vec<_>>()
            .join(", ");

        let mut writes = String::new();

        for (idx, format_element) in self.packet_formats.tx.iter().enumerate() {
            match format_element {
                PacketFormatElement::Crc { algorithm } => {
                    //TODO: crc implementations or library
                    //todo!()
                }
                PacketFormatElement::SizeOfPayload {  size_bits, express_as } => {
                    //TODO: need a write size estimator
                },
                PacketFormatElement::Metadata { segment } => {
                    //TODO
                    let name = segment.get_name();
                    let literal = payload.metadata.get(segment.get_name()).expect("All references to metadata should exist");
                    // write the segments just like we write the payload

                    //TODO: handling for Const
                    //Metadata with Const inside of it is equivalent to just having Const inside packet
                    //format directly. Maybe I can reject this case.

                    let vars = segment.to_c_vars();
                    assert!(vars.len() == 1, "should only have 1 var");
                    writes.push_str(&format!("{INDT}{} = {};\n", vars[0], literal.to_string()));
                    writes.push_str(&self.segment_writes(&vec![segment.clone()]));
                    
                    /*writes.push_str(&match segments {
                        OneOrMany::One(one) => 
                        OneOrMany::Many(many) => self.segment_writes(many)
                    });*/

                },
                PacketFormatElement::Const { data, bits } => {
                    let bits = match bits {
                        Some(bits) => *bits,
                        None => data.len()*8
                    };

                    let data_byte_length = data.len();
                    let data_array = data.iter().map(|e| e.to_string()).collect::<Vec<_>>().join(", ");
                    //index used to prevent name conflicts if there are multiple consts in the header
                    writes.push_str(&formatdoc!("
                    {INDT}byte format_const_{idx}[{data_byte_length}] = {{ {data_array} }};
                    {INDT}device->write(format_const_{idx}, {bits});
                    "));
                },
                PacketFormatElement::Payload => {
                    writes.push_str(&self.segment_writes(&payload.segments))
                },
                PacketFormatElement::Crc { algorithm } => (),
                PacketFormatElement::SizeTotal { size_bits, express_as } => (),
                PacketFormatElement::SizeOfElements { size_bits, express_as, elements } => ()
            }
        }

        

        Ok(formatdoc!("
        \n\n
        // {description}
        void TX{name}(struct Device* device, {args}) {{
        {writes}
        }}"))
    }
    
    pub fn c_emit_rx_function(&self, name: &str, payload: &Payload) -> Result<String, Box<CodegenError>> {
        let description = &payload.description;
        let return_struct_filler = payload.segments.iter()
            .map(|s| s.to_c_vars())
            .flatten()
            .map(|t| format!("{INDT}{t};"))
            .collect::<Vec<_>>()
            .join("\n");

        let return_struct_name = format!("struct RX{name}");

        let return_struct = formatdoc!("
        {return_struct_name} {{ 
        {return_struct_filler} 
        }};");

        Ok(formatdoc!("
        \n\n{return_struct}

        // {description}
        {return_struct_name} RX{name}(struct Device* device) {{
            
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

        for (name, struct_) in self.structs.iter() {
            contents.push_str(&self.emit_struct(struct_))
        }

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
        // make sure references to metadata exist in all packets
        // metadata cannot contain Const, since it's basically already Constant. Use PacketFormatElement::Const instead
        // metadata literals are correct and compatible in each packet
    }
}

impl Payload {
    /// Estimates a payload's size, not including headers etc.
    pub(crate) fn get_size() -> u32 {
        todo!()
    }
}
