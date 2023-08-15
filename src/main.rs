use std::error::Error;
use std::net::SocketAddrV4;
use std::str::FromStr;

use crate::mapper::Mapper;
use crate::mapping::Mapping;

mod mapper;
mod mapping;

fn main() -> Result<(), Box<dyn Error>> {
    let mut mapper = Mapper::new(
        Mapping::apc_mini(),
        SocketAddrV4::from_str("192.168.179.238:7002").unwrap(),
        SocketAddrV4::from_str("192.168.179.238:7001").unwrap(),
        "APC MINI",
    )?;
    mapper.all_midi_off();
    mapper.start();
    Ok(())
}
