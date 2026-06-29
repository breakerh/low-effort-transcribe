use anyhow::{anyhow, Context, Result};
use indicatif::{ProgressBar, ProgressStyle};
use std::fs;
use std::io::{Read, Write};
use std::path::PathBuf;

const HF_BASE: &str = "https://huggingface.co/ggerganov/whisper.cpp/resolve/main";
const MIN_MODEL_SIZE: u64 = 1_000_000;

pub fn ensure_model(name: &str) -> Result<PathBuf> {
    let filename = format!("ggml-{name}.bin");
    let url = format!("{HF_BASE}/{filename}");

    let cache_dir = dirs::cache_dir()
        .context("cannot determine cache directory")?
        .join("transcribe");
    fs::create_dir_all(&cache_dir).context("creating cache directory")?;

    let dest = cache_dir.join(&filename);
    if dest.exists() {
        let size = fs::metadata(&dest)?.len();
        if size > MIN_MODEL_SIZE {
            return Ok(dest);
        }
        // Corrupt / truncated - re-download
        let _ = fs::remove_file(&dest);
    }

    println!("Downloading model: {filename}");
    println!("  url:  {url}");
    println!("  dest: {}", dest.display());

    let tmp = cache_dir.join(format!("{filename}.partial"));
    let _ = fs::remove_file(&tmp);

    let resp = ureq::get(&url).call().context("requesting model")?;

    let total: u64 = resp
        .header("Content-Length")
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);

    let pb = if total > 0 {
        let pb = ProgressBar::new(total);
        pb.set_style(
            ProgressStyle::default_bar()
                .template("  {bar:40.cyan/blue} {bytes}/{total_bytes} {bytes_per_sec} ({eta})")
                .unwrap()
                .progress_chars("=> "),
        );
        pb
    } else {
        ProgressBar::new_spinner()
    };

    let mut reader = resp.into_reader();
    let mut file = fs::File::create(&tmp).context("creating temp file")?;
    let mut buf = vec![0u8; 1 << 20];
    loop {
        let n = reader.read(&mut buf).context("reading model bytes")?;
        if n == 0 {
            break;
        }
        file.write_all(&buf[..n]).context("writing model bytes")?;
        pb.inc(n as u64);
    }
    file.flush().context("flushing model file")?;
    drop(file);
    pb.finish();

    let downloaded = fs::metadata(&tmp)?.len();
    if downloaded < MIN_MODEL_SIZE {
        let _ = fs::remove_file(&tmp);
        return Err(anyhow!(
            "downloaded model is only {downloaded} bytes - check model name '{name}' and network"
        ));
    }

    fs::rename(&tmp, &dest).context("finalising model file")?;
    Ok(dest)
}
