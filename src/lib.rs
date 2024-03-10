#[macro_use]
extern crate derive_more;
extern crate rand;

pub mod config;
pub mod docgen;

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
    /// Returns variables necessary for sending/recieving this field. 
    /// 
    /// # Rreturns
    /// If Self is a constant, no variables are
    /// necessary since the field will be checked & discarded on recieve or hardcoded on transmit,
    /// so returns an empty array in this case. 
    ///
    /// If Self is an array type that is unsized, we will need a length to describe it. In this
    /// case, will return a two element Vec. 
    ///
    /// Will never return more than 2 elements. If it returns a two element vec, the second element
    /// is the length variable requirement
    ///
    /// # Arguments
    /// * `static_arrays` - If this is to be used as a "local_var". In particular, will make
    /// known-sized arrays in static declaration syntax
    /// * `skip_const` - Whether to skip Const data types (for example, when generating
    /// arguments or return values)
    fn get_necessary_c_vars(&self, static_arrays: bool, skip_const: bool) -> Vec<String> {
        match self {
            PacketSegment::Sized { name, bits, datatype } => {
                let type_name = match datatype {
                    SizedDataType::StringUTF8 => {
                        if static_arrays {
                            //TODO: panic on non-byte divisible strings, since we don't have a
                            //well-defined way of handling this. Also check for this in validator
                            return vec![ format!("char {name}[{total}]", total = bits/8) ]
                        }

                        "char*".to_owned()
                    },
                    // TODO: bitshifting for non-multiples-of-8-bit-types
                    SizedDataType::Integer { endianness: _, signing } => format!("{sign}int{bits}_t", sign = match signing {
                        Signing::OnesComplement | Signing::TwosComplement => "",
                        Signing::Unsigned => "u"
                    }),
                    SizedDataType::Raw => {
                        if static_arrays {
                            //TODO: ceiling function for non-8-bit-multiple lengths
                            return vec![ format!("uint8_t {name}[{total}]", total = bits/8) ]
                        }

                        "uint8_t*".to_owned()
                    },
                    //should be skipped, since we will hardcode the contents
                    SizedDataType::Const { data } => {
                        //TODO include Const
                        if skip_const {
                            return vec![];
                        }

                        if static_arrays {
                            //TODO: ceiling function for non-8-bit-multiple lengths
                            return vec![ format!("uint8_t {name}[{}]", data.len()) ];
                        } 

                        format!("uint8_t*")
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
                        vec![format!("uint8_t* {name}"), format!("size_t {name}_length")]
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
            .map(|s| s.get_necessary_c_vars(true, true))
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
                            writes.push_str(&format!("{INDT}device->write((uint8_t*) &{name}, {bits});\n"));
                        },
                        SizedDataType::Raw => {
                            // just write the argument straight up
                            writes.push_str(&format!("{INDT}device->write({name}, {bits});\n"));
                        },
                        SizedDataType::StringUTF8 => {
                            writes.push_str(&format!("{INDT}device->write((uint8_t*) {name}, {bits});\n"));
                        },
                        SizedDataType::FloatIEEE { endianness } => {
                            //TODO set endianness
                            writes.push_str(&format!("{INDT}device->write((uint8_t*) &{name}, {bits});\n"));
                        },
                        SizedDataType::Const { data } => {
                            let data_byte_length = data.len();
                            let data_array = data.iter().map(|e| e.to_string()).collect::<Vec<_>>().join(", ");
                            writes.push_str(&formatdoc!("
                            {INDT}uint8_t {name}[{data_byte_length}] = {{ {data_array} }};
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

                            let vars = target_segment.get_necessary_c_vars(false,false);

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
                            uint8_t* {name}_terminator = {{ {data_array} }};
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
        let mut args = payload.segments.iter()
            .map(|s| s.get_necessary_c_vars(false, true))
            .flatten()
            .collect::<Vec<_>>();

        args.insert(0, "struct Device* device".to_owned());
        let args = args.join(", ");

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
                    //TODO: Support for dynamic types and lists that require us to populate
                    //multiple variables
                    let name = segment.get_name();
                    let literal = payload.metadata.get(segment.get_name()).expect("All references to metadata should exist");
                    // write the segments just like we write the payload

                    //TODO: handling for Const
                    //Metadata with Const inside of it is equivalent to just having Const inside packet
                    //format directly. Maybe I can reject this case.

                    // maybe make the get_necessary_c_vars have an option for initalization
                    let vars = segment.get_necessary_c_vars(true, false);

                    match (vars.len(), literal) {
                        (1, OneOrMany::One(literal)) => {
                            writes.push_str(&format!("{INDT}{} = {};\n", vars[0], literal.to_string()));
                        },
                        (1, OneOrMany::Many(_)) => {
                            //TODO: may be allowed in some cases, i.e. fixed-size arrays. Figure
                            //out if this is true and make the behavior correct
                            unimplemented!("Expected one item, but got several. This may become possible in the future for fized-and-known-size elements");
                        }
                        (2, OneOrMany::Many(literal)) => {
                            // assumption: if to_c_vars() returns two variables, the second is the length
                            // variable. This is documented in the spec for get_necessary_c_vars
                            // TODO: fix this. Probably use our own untagged Value struct to keep
                            // things sane
                            writes.push_str(&format!("{INDT}{} = {{ {} }};\n", vars[0], literal.iter().map(|v| v.to_string()).collect::<Vec<_>>().join(", ") ));

                            // the second required var is the length 
                            // TODO
                            writes.push_str(&format!("{INDT}{} = {}*8;\n", vars[1], literal.len()));
                        },
                        (2, OneOrMany::One(o)) => {
                            panic!("TODO: error handling. One found when many expected. Please covert to an array ( {o} -> [{o}] )", o = o.to_string());
                        }
                        _ => {
                            panic!("More than 2 variables came back! ");
                        }
                    }

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
                    {INDT}uint8_t format_const_{idx}[{data_byte_length}] = {{ {data_array} }};
                    {INDT}device->write(format_const_{idx}, {bits});
                    "));
                },
                PacketFormatElement::Payload => {
                    writes.push_str(&self.segment_writes(&payload.segments))
                },
                PacketFormatElement::Crc { algorithm } => (), //TODO
                PacketFormatElement::SizeTotal { size_bits, express_as } => (),//TODO
                PacketFormatElement::SizeOfElements { size_bits, express_as, elements } => ()//TODO
            }
        }

        

        Ok(formatdoc!("
        \n\n
        /// {description}
        void TX{name}({args}) {{
        {writes}
        }}"))
    }
    
    pub fn c_emit_rx_function(&self, name: &str, payload: &Payload) -> Result<String, Box<CodegenError>> {
        let description = &payload.description;
        let return_struct_filler = payload.segments.iter()
            .map(|s| s.get_necessary_c_vars(true, true))
            .flatten()
            .map(|t| format!("{INDT}{t};"))
            .collect::<Vec<_>>()
            .join("\n");

        let return_struct_name = format!("struct RX{name}");

        let return_struct = formatdoc!("
        {return_struct_name} {{ 
        {return_struct_filler} 
        }};");

        let mut reads = String::new();

        for (idx, segment) in payload.segments.iter().enumerate() {
            match segment {
                PacketSegment::Sized { name, bits, datatype } => {
                    match datatype {
                        SizedDataType::Raw => {
                            reads.push_str("device->read((uint8_t*)&ret.{name}, {bits});\n");
                        },
                        SizedDataType::Const { data } => {
                            reads.push_str(&format!("{INDT}// Read the \"constant\" data in so we can compare it"));
                            reads.push_str(&format!("{INDT}uint8_t actual_const_{idx}[{len}];\n", len = data.len()));
                            reads.push_str(&format!("{INDT}uint8_t expected_const_{idx}[{len}] = {{ {array} }}\n", len = data.len(), array = data.iter().map(|d| d.to_string()).collect::<Vec<_>>().join(", ")));
                            reads.push_str(&format!("{INDT}device->read((uint8_t*)&ret.{name}, {bits});\n"));
                            reads.push_str(&format!("{INDT}//TODO: assert that this is the same as the expected const"));
                            reads.push_str(&format!("{INDT}for(int i=0; i<{len}; i++) {{\n{INDT}assert(expected_const_{idx}[i] == actual_const_{idx}[i])\n{INDT}}}\n", len = data.len()));
                        },
                        SizedDataType::Integer { endianness, signing } => {
                            reads.push_str(&format!("{INDT}device->read((uint8_t*)&ret.{name}, {bits});\n"));
                            //TODO: process one's complement and endianness
                        },
                        SizedDataType::StringUTF8 => {
                            reads.push_str(&format!("\n{INDT}device->read((uint8_t*)&ret.{name}, {bits})\n"));
                            reads.push_str(&format!("{INDT}ret.{name}[{last_char}] = '\\0')\n", last_char = bits/8));
                            //TODO null-terminate the string
                            //TODO: make sure the struct string type has enough space for null
                            //terminator
                        },
                        SizedDataType::FloatIEEE { endianness } => {
                            //TODO endianness
                            reads.push_str(&format!("{INDT}device->read((uint8_t*)&ret.{name}, {bits})\n"));
                        }
                    }
                },
                PacketSegment::Unsized { name, datatype, termination } => {
                    match datatype {
                        UnsizedDataType::Raw => {
                            //TODO
                        },
                        UnsizedDataType::Array { item_struct } => {
                            //TODO
                        },
                        UnsizedDataType::StringUTF8 => {
                            //TODO
                        }
                    }
                },
                PacketSegment::Struct { name, struct_name } => {
                    unimplemented!()
                }
            }
        }

        Ok(formatdoc!("
        \n\n{return_struct}

        /// {description}
        {return_struct_name} RX{name}(struct Device* device) {{
        {INDT}{return_struct_name} ret;
        {reads} 
        {INDT}return ret;
        }}"))
    }

    /// Generates C program code to the given destination directory
    pub fn codegen_linux_c(&self, destination: std::path::PathBuf) -> Result<(), Box<CodegenError>> {
        println!("{:?}",destination.exists());
        let file = destination.join("lib.c");
        let mut contents = formatdoc!("
            #include <stdio.h>
            #include <stdlib.h>
            #include <ctype.h>
            #include <assert.h>

            struct Device {{
            {INDT}// Writes data with length to the device, returning bytes written, or a negative
            {INDT}// number for an error. Should block until entire write is complete
            {INDT}int (*write)(uint8_t* data, size_t length_bits);

            {INDT}// Reads data with max length from the device, returning bytes read or a negative
            {INDT}// number for an error. Should block until read is complete
            {INDT}int (*read)(uint8_t* data, size_t length_bits);
            }};");

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
