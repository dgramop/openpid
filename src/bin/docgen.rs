extern crate mdbook;
use mdbook::MDBook;
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    std::fs::create_dir_all("outputs/book");

    let mut cfg = mdbook::config::Config::default();

    cfg.book.title = Some("Sensor Title".to_string());

    MDBook::init("outputs/book")
        .with_config(cfg)
        .build()?;

    std::fs::write("outputs/book/image.svg", openpid::docgen::generate_packet_diagram("Packet Format".to_owned(), vec![("Size".to_owned(), Some(8)), ("FrameID".to_owned(), Some(8)),("Payload".to_owned(), None), ("Crc".to_owned(), Some(16))]))?;
    Ok(())
}
