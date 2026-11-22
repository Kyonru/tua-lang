use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{fs::File, io};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildInfo {
    pub schema_version: u32,
    pub version: &'static str,
    pub revision: Option<&'static str>,
    pub dirty: Option<bool>,
    pub target: &'static str,
    pub profile: &'static str,
    pub executable_sha256: Option<String>,
}

pub fn build_info() -> BuildInfo {
    let revision = env!("TUA_BUILD_REVISION");
    BuildInfo {
        schema_version: 1,
        version: tua_core::VERSION,
        revision: (!revision.is_empty()).then_some(revision),
        dirty: env!("TUA_BUILD_DIRTY").parse().ok(),
        target: env!("TUA_BUILD_TARGET"),
        profile: env!("TUA_BUILD_PROFILE"),
        executable_sha256: executable_hash().ok(),
    }
}

fn executable_hash() -> io::Result<String> {
    let mut file = File::open(std::env::current_exe()?)?;
    let mut hash = Sha256::new();
    io::copy(&mut file, &mut hash)?;
    Ok(format!("{:x}", hash.finalize()))
}
