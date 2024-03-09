#[macro_use]
extern crate derive_more;

pub mod config;
use crate::config::OpenPID;

pub mod prelude {
    pub use crate::config::*;
}

#[derive(Debug, Display)]
pub enum CodegenError {
    IOError(std::io::Error)
}

impl From<std::io::Error> for Box<CodegenError> {
    fn from(value: std::io::Error) -> Self {
        Box::new(CodegenError::IOError(value))
    }
}

impl OpenPID {
    /// Generates C program code to the given destination directory
    pub fn codegen_linux_c(&self, destination: std::path::PathBuf) -> Result<(), Box<CodegenError>> {
        println!("{:?}",destination.exists());
        let file = destination.join("lib.c");

        let contents = String::from("#include <stdio>\n#include <stdlib>");

        std::fs::write(file, contents)?;

        Ok(())
    }
}
