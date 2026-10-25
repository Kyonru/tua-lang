mod server;

pub use server::{run_stdio, server_description, server_name};

#[cfg(feature = "fuzzing")]
pub fn fuzz_transport(input: &[u8]) {
    server::fuzz_transport(input);
}
