#[macro_use]
extern crate serde;

pub mod transferpb {
    tonic::include_proto!("transferpb");
}

pub mod client;
pub mod server;
