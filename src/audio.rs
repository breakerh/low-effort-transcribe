use anyhow::{anyhow, Context, Result};
use rubato::{
    Resampler, SincFixedIn, SincInterpolationParameters, SincInterpolationType, WindowFunction,
};
use std::fs::File;
use std::path::Path;
use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::{DecoderOptions, CODEC_TYPE_NULL};
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

const TARGET_RATE: u32 = 16_000;

pub fn decode_to_16k_mono(path: &Path) -> Result<Vec<f32>> {
    let file = File::open(path).with_context(|| format!("opening {}", path.display()))?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());

    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }

    let probed = symphonia::default::get_probe()
        .format(
            &hint,
            mss,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )
        .context("probing media format")?;

    let mut format = probed.format;
    let track = format
        .tracks()
        .iter()
        .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
        .context("no decodable audio track in file")?
        .clone();
    let track_id = track.id;

    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .context("creating audio decoder")?;

    let mut mono: Vec<f32> = Vec::new();
    let mut source_rate: Option<u32> = None;

    loop {
        let packet = match format.next_packet() {
            Ok(p) => p,
            Err(SymphoniaError::IoError(e))
                if e.kind() == std::io::ErrorKind::UnexpectedEof =>
            {
                break;
            }
            Err(SymphoniaError::ResetRequired) => break,
            Err(e) => return Err(e.into()),
        };
        if packet.track_id() != track_id {
            continue;
        }

        let decoded = match decoder.decode(&packet) {
            Ok(d) => d,
            Err(SymphoniaError::DecodeError(_)) => continue,
            Err(e) => return Err(e.into()),
        };

        let spec = *decoded.spec();
        if source_rate.is_none() {
            source_rate = Some(spec.rate);
        }
        let channels = spec.channels.count();

        let mut sample_buf = SampleBuffer::<f32>::new(decoded.capacity() as u64, spec);
        sample_buf.copy_interleaved_ref(decoded);
        let interleaved = sample_buf.samples();

        if channels == 1 {
            mono.extend_from_slice(interleaved);
        } else {
            for chunk in interleaved.chunks_exact(channels) {
                let sum: f32 = chunk.iter().sum();
                mono.push(sum / channels as f32);
            }
        }
    }

    if mono.is_empty() {
        return Err(anyhow!("no audio samples were decoded"));
    }

    let source_rate = source_rate.context("could not determine source sample rate")?;
    if source_rate != TARGET_RATE {
        mono = resample(&mono, source_rate, TARGET_RATE)?;
    }

    Ok(mono)
}

fn resample(input: &[f32], from: u32, to: u32) -> Result<Vec<f32>> {
    let params = SincInterpolationParameters {
        sinc_len: 256,
        f_cutoff: 0.95,
        interpolation: SincInterpolationType::Linear,
        oversampling_factor: 128,
        window: WindowFunction::BlackmanHarris2,
    };

    let ratio = to as f64 / from as f64;
    let chunk_size = 1024usize;
    let mut resampler = SincFixedIn::<f32>::new(ratio, 2.0, params, chunk_size, 1)
        .context("creating resampler")?;

    let estimated = (input.len() as f64 * ratio) as usize + chunk_size;
    let mut output: Vec<f32> = Vec::with_capacity(estimated);

    let mut pos = 0usize;
    while pos + chunk_size <= input.len() {
        let chunk = &input[pos..pos + chunk_size];
        let out = resampler
            .process(&[chunk], None)
            .context("resampling chunk")?;
        output.extend_from_slice(&out[0]);
        pos += chunk_size;
    }

    if pos < input.len() {
        let mut last = vec![0.0f32; chunk_size];
        let tail = &input[pos..];
        last[..tail.len()].copy_from_slice(tail);
        let out = resampler
            .process(&[&last], None)
            .context("resampling tail")?;
        let valid_len = ((tail.len() as f64) * ratio).ceil() as usize;
        let take = valid_len.min(out[0].len());
        output.extend_from_slice(&out[0][..take]);
    }

    Ok(output)
}
