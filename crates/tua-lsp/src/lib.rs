mod server;

pub use server::{run_stdio, run_stdio_with_shader_validator, server_description, server_name};

#[cfg(feature = "fuzzing")]
pub fn fuzz_transport(input: &[u8]) {
    server::fuzz_transport(input);
}
