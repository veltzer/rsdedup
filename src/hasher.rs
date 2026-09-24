use anyhow::Result;
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::Read;
use std::path::Path;

use crate::types::HashAlgo;

const PARTIAL_HASH_SIZE: usize = 4096;

pub fn hash_file(path: &Path, algo: HashAlgo, partial: bool) -> Result<String> {
    let mut file = File::open(path)?;

    if partial {
        let mut buf = vec![0u8; PARTIAL_HASH_SIZE];
        let n = file.read(&mut buf)?;
        buf.truncate(n);
        return Ok(hash_bytes(&buf, algo));
    }

    match algo {
        HashAlgo::Sha256 => {
            let mut hasher = Sha256::new();
            let mut buf = [0u8; 65536];
            loop {
                let n = file.read(&mut buf)?;
                if n == 0 {
                    break;
                }
                hasher.update(&buf[..n]);
            }
            Ok(to_hex(&hasher.finalize()))
        }
        HashAlgo::Xxhash => {
            let mut buf = Vec::new();
            file.read_to_end(&mut buf)?;
            let hash = xxhash_rust::xxh3::xxh3_128(&buf);
            Ok(format!("{hash:032x}"))
        }
        HashAlgo::Blake3 => {
            let mut hasher = blake3::Hasher::new();
            let mut buf = [0u8; 65536];
            loop {
                let n = file.read(&mut buf)?;
                if n == 0 {
                    break;
                }
                hasher.update(&buf[..n]);
            }
            Ok(hasher.finalize().to_hex().to_string())
        }
    }
}

fn hash_bytes(data: &[u8], algo: HashAlgo) -> String {
    match algo {
        HashAlgo::Sha256 => {
            let mut hasher = Sha256::new();
            hasher.update(data);
            to_hex(&hasher.finalize())
        }
        HashAlgo::Xxhash => {
            let hash = xxhash_rust::xxh3::xxh3_128(data);
            format!("{hash:032x}")
        }
        HashAlgo::Blake3 => {
            let hash = blake3::hash(data);
            hash.to_hex().to_string()
        }
    }
}

/// Lowercase hex encoding of a digest. sha2 0.11 dropped the `LowerHex`
/// impl on its output array, so the encoding lives here.
fn to_hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    bytes
        .iter()
        .fold(String::with_capacity(bytes.len() * 2), |mut s, b| {
            let _ = write!(s, "{b:02x}");
            s
        })
}
