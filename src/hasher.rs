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
            std::io::copy(&mut file, &mut hasher)?;
            Ok(format!("{:x}", hasher.finalize()))
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
            format!("{:x}", hasher.finalize())
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
