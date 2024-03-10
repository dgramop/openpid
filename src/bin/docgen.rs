
fn main() {
    println!("{}", openpid::docgen::generate_packet_diagram("Packet Format".to_owned(), vec![("Size".to_owned(), Some(8)), ("FrameID".to_owned(), Some(8)),("Payload".to_owned(), None), ("Crc".to_owned(), Some(16))]))
}
