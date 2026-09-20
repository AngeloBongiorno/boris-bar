use std::{env, fs, path::PathBuf};

const CLIPS: &[&str] = &["fai_uno_sforzo", "tutti_basiti", "a_cazzo_di_cane", "f4"];

fn main() {
    let out = PathBuf::from(env::var("OUT_DIR").unwrap());
    println!("cargo:rerun-if-changed=assets/clips");

    for name in CLIPS {
        let file = format!("{name}.mp3");
        let src = PathBuf::from("assets/clips").join(&file);
        let dst = out.join(&file);

        if src.exists() {
            fs::copy(&src, &dst).expect("copy clip");
        } else {
            println!("cargo:warning=missing {}, using silence", src.display());
            fs::write(&dst, silent_wav()).expect("write placeholder");
        }
    }
}

/// 0.1s of 16-bit mono silence at 44.1kHz.
fn silent_wav() -> Vec<u8> {
    let samples = 4410usize;
    let data_len = samples * 2;
    let mut w = Vec::with_capacity(44 + data_len);
    w.extend_from_slice(b"RIFF");
    w.extend_from_slice(&((36 + data_len) as u32).to_le_bytes());
    w.extend_from_slice(b"WAVEfmt ");
    w.extend_from_slice(&16u32.to_le_bytes()); // fmt chunk size
    w.extend_from_slice(&1u16.to_le_bytes()); // PCM
    w.extend_from_slice(&1u16.to_le_bytes()); // mono
    w.extend_from_slice(&44100u32.to_le_bytes());
    w.extend_from_slice(&88200u32.to_le_bytes()); // byte rate
    w.extend_from_slice(&2u16.to_le_bytes()); // block align
    w.extend_from_slice(&16u16.to_le_bytes()); // bits per sample
    w.extend_from_slice(b"data");
    w.extend_from_slice(&(data_len as u32).to_le_bytes());
    w.resize(44 + data_len, 0);
    w
}
