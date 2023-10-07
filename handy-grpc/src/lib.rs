#[macro_use]
extern crate serde;

pub mod transferpb {
    tonic::include_proto!("transferpb");
}

pub type Priority = u8;

pub mod client;
pub mod server;
