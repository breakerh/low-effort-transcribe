use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::time::Instant;
use walkdir::WalkDir;
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

use crate::{audio, model};

pub struct Config {
    pub path: PathBuf,
    pub language: String,
    pub model: String,
    pub force: bool,
}

#[derive(Debug, Clone)]
pub enum Event {
    NoFilesFound { path: PathBuf },
    FilesFound { count: usize },
    ModelLoading { path: PathBuf },
    FileStart { index: usize, total: usize, path: PathBuf },
    FileSkipped { index: usize, total: usize, path: PathBuf },
    FileDecoded { samples: usize, duration_s: f32, elapsed_s: f32 },
    FileTranscribed { txt_path: PathBuf, chars: usize, elapsed_s: f32 },
    FileError { path: PathBuf, error: String },
    Done,
}

pub fn run(config: Config, mut on_event: impl FnMut(Event)) -> Result<()> {
    let files = find_media(&config.path)?;
    if files.is_empty() {
        on_event(Event::NoFilesFound {
            path: config.path.clone(),
        });
        on_event(Event::Done);
        return Ok(());
    }
    on_event(Event::FilesFound { count: files.len() });

    let model_path = model::ensure_model(&config.model)?;
    on_event(Event::ModelLoading {
        path: model_path.clone(),
    });

    let model_str = model_path
        .to_str()
        .context("model path is not valid UTF-8")?;
    let ctx = WhisperContext::new_with_params(model_str, WhisperContextParameters::default())
        .context("loading whisper model")?;

    let n_threads = std::thread::available_parallelism()
        .map(|n| n.get() as i32)
        .unwrap_or(4);

    let total = files.len();
    for (i, file) in files.iter().enumerate() {
        let txt_path = file.with_extension("txt");
        if txt_path.exists() && !config.force {
            on_event(Event::FileSkipped {
                index: i,
                total,
                path: file.clone(),
            });
            continue;
        }

        on_event(Event::FileStart {
            index: i,
            total,
            path: file.clone(),
        });

        let started = Instant::now();
        let samples = match audio::decode_to_16k_mono(file) {
            Ok(s) => s,
            Err(e) => {
                on_event(Event::FileError {
                    path: file.clone(),
                    error: format!("{e:#}"),
                });
                continue;
            }
        };
        on_event(Event::FileDecoded {
            samples: samples.len(),
            duration_s: samples.len() as f32 / 16_000.0,
            elapsed_s: started.elapsed().as_secs_f32(),
        });

        let t0 = Instant::now();
        let mut state = ctx.create_state().context("creating whisper state")?;
        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
        if config.language != "auto" {
            params.set_language(Some(&config.language));
        }
        params.set_n_threads(n_threads);
        params.set_translate(false);
        params.set_no_context(true);
        params.set_suppress_blank(true);
        params.set_no_speech_thold(0.6);
        params.set_temperature(0.0);
        params.set_entropy_thold(2.4);
        params.set_logprob_thold(-1.0);
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_special(false);
        params.set_print_timestamps(false);

        state
            .full(params, &samples)
            .context("running whisper")?;

        let n = state.full_n_segments().context("counting segments")?;
        let mut segments: Vec<(i64, i64, String)> = Vec::with_capacity(n as usize);
        for j in 0..n {
            let text = state
                .full_get_segment_text(j)
                .context("reading segment text")?;
            let seg_t0 = state.full_get_segment_t0(j).unwrap_or(0);
            let seg_t1 = state.full_get_segment_t1(j).unwrap_or(seg_t0);
            segments.push((seg_t0, seg_t1, text));
        }
        let transcript = build_transcript(&segments);

        std::fs::write(&txt_path, &transcript)
            .with_context(|| format!("writing {}", txt_path.display()))?;
        on_event(Event::FileTranscribed {
            txt_path,
            chars: transcript.len(),
            elapsed_s: t0.elapsed().as_secs_f32(),
        });
    }

    on_event(Event::Done);
    Ok(())
}

pub fn find_media(root: &Path) -> Result<Vec<PathBuf>> {
    if root.is_file() {
        return Ok(if is_supported(root) {
            vec![root.to_path_buf()]
        } else {
            Vec::new()
        });
    }
    let mut out = Vec::new();
    for entry in WalkDir::new(root).into_iter().filter_map(|e| e.ok()) {
        if entry.file_type().is_file() && is_supported(entry.path()) {
            out.push(entry.into_path());
        }
    }
    out.sort();
    Ok(out)
}

fn is_supported(path: &Path) -> bool {
    match path.extension().and_then(|e| e.to_str()) {
        Some(ext) => {
            let lower = ext.to_ascii_lowercase();
            lower == "mp3" || lower == "mp4"
        }
        None => false,
    }
}

/// Builds the transcript from collected whisper segments:
/// - skips empty/whitespace segments
/// - skips a segment that is identical to the previous one (dedupes hallucination loops)
/// - joins segments as flowing prose; inserts a blank line on pauses longer than 2s
pub fn build_transcript(segments: &[(i64, i64, String)]) -> String {
    let mut out = String::new();
    let mut last_text: Option<String> = None;
    let mut last_end: i64 = 0;

    for (start, end, text) in segments {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            continue;
        }
        if last_text.as_deref() == Some(trimmed) {
            continue;
        }

        if !out.is_empty() {
            // segment times are in 10ms units; 200 = 2 seconds
            if *start - last_end > 200 {
                out.push_str("\n\n");
            } else {
                out.push(' ');
            }
        }

        out.push_str(trimmed);
        last_text = Some(trimmed.to_string());
        last_end = *end;
    }

    if !out.is_empty() {
        out.push('\n');
    }
    out
}
