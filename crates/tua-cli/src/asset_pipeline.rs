use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Component, Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::DecoderOptions;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;
use tua_core::{
    AssetPipelineConfig, Diagnostic, DiagnosticCode, DiagnosticSeverity, ProjectConfig,
};

use crate::path_security::reject_symlink_path;
use crate::source::io_diagnostic;

const MANIFEST_VERSION: u32 = 1;
const AUDIO_ANALYZER_VERSION: &str = "3";
const MAX_IMPORTER_TIMEOUT_MS: u64 = 3_600_000;
const MAX_MANIFEST_BYTES: u64 = 16 * 1024 * 1024;
const MAX_STDERR_BYTES: u64 = 1024 * 1024;
const MAX_AUDIO_DURATION_SECONDS: u64 = 3_600;
const MAX_AUDIO_SAMPLE_RATE: u32 = 192_000;
const MAX_AUDIO_CHANNELS: usize = 8;
const MAX_AUDIO_ANALYSIS_POINTS: usize = 1_000_000;
const MAX_AUDIO_PACKET_FRAMES: usize = 1_048_576;
const FNV_OFFSET_BASIS: u64 = 0xcbf29ce484222325;
const FNV_PRIME: u64 = 0x100000001b3;

#[derive(Debug, Default)]
pub(crate) struct AssetPipelineOutcome {
    pub(crate) diagnostics: Vec<Diagnostic>,
    pub(crate) executed: usize,
    pub(crate) cached: usize,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ImportManifest {
    version: u32,
    #[serde(default)]
    outputs: Vec<String>,
    #[serde(default)]
    diagnostics: Vec<ImportDiagnostic>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ImportDiagnostic {
    severity: String,
    message: String,
    #[serde(default)]
    path: String,
    #[serde(default = "one")]
    line: usize,
    #[serde(default = "one")]
    column: usize,
    #[serde(default)]
    end_line: Option<usize>,
    #[serde(default)]
    end_column: Option<usize>,
    #[serde(default)]
    code: Option<u16>,
}

#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct MaterializedState {
    #[serde(default)]
    outputs: Vec<String>,
}

fn one() -> usize {
    1
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AudioAnalysis {
    path: String,
    duration: f64,
    channels: usize,
    sample_rate: u32,
    envelope_hz: f64,
    rms: f64,
    peak: f64,
    bpm: Option<f64>,
    confidence: f64,
    tempo_candidates: Vec<TempoCandidate>,
    tempo_ambiguity: f64,
    boundary_influence: f64,
    beat_offset: f64,
    beat_grid: Vec<f64>,
    energy: Vec<f64>,
    low: Vec<f64>,
    mid: Vec<f64>,
    high: Vec<f64>,
    onset: Vec<f64>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct TempoCandidate {
    bpm: f64,
    score: f64,
    relative_score: f64,
    relation: String,
}

#[derive(Clone, Debug)]
struct TempoEstimate {
    bpm: Option<f64>,
    confidence: f64,
    beat_offset: f64,
    candidates: Vec<TempoCandidate>,
    ambiguity: f64,
    boundary_influence: f64,
}

fn option_number(pipeline: &AssetPipelineConfig, name: &str, fallback: f64) -> f64 {
    pipeline
        .options
        .get(name)
        .and_then(|value| value.as_f64())
        .unwrap_or(fallback)
}

fn execute_audio_analysis(
    cwd: &Path,
    pipeline: &AssetPipelineConfig,
    inputs: &[PathBuf],
    output: &Path,
) -> Result<ImportManifest, Vec<Diagnostic>> {
    let envelope_hz = option_number(pipeline, "envelope_hz", 20.0).clamp(1.0, 200.0);
    let tempo_min = option_number(pipeline, "tempo_min", 60.0).clamp(20.0, 400.0);
    let tempo_max = option_number(pipeline, "tempo_max", 200.0).clamp(tempo_min, 400.0);
    let confidence_warning = option_number(pipeline, "confidence_warning", 0.35).clamp(0.0, 1.0);
    let mut analyses = Vec::new();
    let mut diagnostics = Vec::new();
    let mut remaining_analysis_points = MAX_AUDIO_ANALYSIS_POINTS;

    for input in inputs {
        match analyze_audio_file(
            cwd,
            input,
            envelope_hz,
            tempo_min,
            tempo_max,
            remaining_analysis_points,
        ) {
            Ok(analysis) => {
                remaining_analysis_points =
                    remaining_analysis_points.saturating_sub(analysis.energy.len());
                if analysis.bpm.is_none() || analysis.confidence < confidence_warning {
                    let candidates = analysis
                        .tempo_candidates
                        .iter()
                        .map(|candidate| format!("{:.1}", candidate.bpm))
                        .collect::<Vec<_>>()
                        .join(", ");
                    let detail = if candidates.is_empty() {
                        String::new()
                    } else {
                        format!(
                            " (ambiguity {:.2}; candidates {candidates} BPM)",
                            analysis.tempo_ambiguity
                        )
                    };
                    diagnostics.push(ImportDiagnostic {
                        severity: "warning".to_owned(),
                        message: format!("tempo confidence is low{detail}; author BPM and beat offset before using quantized transitions"),
                        path: analysis.path.clone(), line: 1, column: 1,
                        end_line: None, end_column: None, code: Some(2012),
                    });
                }
                if analysis.channels > 1 {
                    diagnostics.push(ImportDiagnostic {
                        severity: "info".to_owned(),
                        message: "stereo audio cannot use LÖVE positional playback".to_owned(),
                        path: analysis.path.clone(),
                        line: 1,
                        column: 1,
                        end_line: None,
                        end_column: None,
                        code: Some(2012),
                    });
                }
                analyses.push(analysis);
            }
            Err(message) => diagnostics.push(ImportDiagnostic {
                severity: "error".to_owned(),
                message,
                path: display_path(input.strip_prefix(cwd).unwrap_or(input)),
                line: 1,
                column: 1,
                end_line: None,
                end_column: None,
                code: Some(2012),
            }),
        }
    }

    if diagnostics.iter().any(|item| item.severity == "error") {
        return Err(manifest_diagnostics(
            cwd,
            pipeline,
            &ImportManifest {
                version: MANIFEST_VERSION,
                outputs: Vec::new(),
                diagnostics,
            },
        ));
    }
    analyses.sort_by(|left, right| left.path.cmp(&right.path));
    let data_lua = audio_catalog_lua(&analyses);
    let module = pipeline
        .output
        .components()
        .skip(1)
        .map(|part| part.as_os_str().to_string_lossy())
        .chain(std::iter::once(std::borrow::Cow::Borrowed("catalog_data")))
        .collect::<Vec<_>>()
        .join(".");
    let facade = format!(
        r#"export type AudioTempoCandidate = {{
  bpm: number,
  score: number,
  relativeScore: number,
  relation: string,
}}

export type AudioAnalysis = {{
  path: string,
  duration: number,
  channels: number,
  sampleRate: number,
  envelopeHz: number,
  rms: number,
  peak: number,
  bpm: number | nil,
  confidence: number,
  tempoCandidates: AudioTempoCandidate[],
  tempoAmbiguity: number,
  boundaryInfluence: number,
  beatOffset: number,
  beatGrid: number[],
  energy: number[],
  low: number[],
  mid: number[],
  high: number[],
  onset: number[],
}}

local catalog: {{[string]: AudioAnalysis }} = require("{module}")
return catalog
"#
    );
    let index = serde_json::to_vec_pretty(&analyses)
        .map_err(|error| vec![pipeline_error(cwd, pipeline, error.to_string())])?;
    for (name, bytes) in [
        ("catalog.tua", facade.into_bytes()),
        ("catalog_data.lua", data_lua.into_bytes()),
        ("audio-index.json", index),
    ] {
        let path = output.join(name);
        fs::write(&path, bytes).map_err(|source| vec![io_diagnostic(&path, source)])?;
    }
    Ok(ImportManifest {
        version: MANIFEST_VERSION,
        outputs: vec![
            "catalog.tua".to_owned(),
            "catalog_data.lua".to_owned(),
            "audio-index.json".to_owned(),
        ],
        diagnostics,
    })
}

fn analyze_audio_file(
    cwd: &Path,
    path: &Path,
    envelope_hz: f64,
    tempo_min: f64,
    tempo_max: f64,
    max_analysis_points: usize,
) -> Result<AudioAnalysis, String> {
    let file = File::open(path).map_err(|error| error.to_string())?;
    let stream = MediaSourceStream::new(Box::new(file), Default::default());
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default();
    let mut hint = Hint::new();
    hint.with_extension(extension);
    let probed = symphonia::default::get_probe()
        .format(
            &hint,
            stream,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )
        .map_err(|error| format!("unsupported or corrupt audio: {error}"))?;
    let mut format = probed.format;
    let track = format
        .default_track()
        .ok_or_else(|| "audio contains no default track".to_owned())?;
    let track_id = track.id;
    let params = track.codec_params.clone();
    let sample_rate = params
        .sample_rate
        .ok_or_else(|| "audio sample rate is unknown".to_owned())?;
    let channels = params
        .channels
        .map(|channels| channels.count())
        .unwrap_or(1);
    let max_frames = audio_frame_limit(sample_rate, channels, params.n_frames)?;
    let mut decoder = symphonia::default::get_codecs()
        .make(&params, &DecoderOptions::default())
        .map_err(|error| format!("audio codec is unsupported: {error}"))?;
    let window = ((sample_rate as f64 / envelope_hz).round() as usize).max(1);
    let mut energy = Vec::new();
    let mut low = Vec::new();
    let mut mid = Vec::new();
    let mut high = Vec::new();
    let mut peak = 0.0_f64;
    let mut square_sum = 0.0_f64;
    let low_alpha = (-2.0 * std::f64::consts::PI * 200.0 / sample_rate as f64).exp();
    let mid_alpha = (-2.0 * std::f64::consts::PI * 2000.0 / sample_rate as f64).exp();
    let mut low_pass = 0.0_f64;
    let mut mid_pass = 0.0_f64;
    let mut broad_sum = 0.0_f64;
    let mut low_sum = 0.0_f64;
    let mut mid_sum = 0.0_f64;
    let mut high_sum = 0.0_f64;
    let mut window_len = 0_usize;
    let mut decoded_frames = 0_u64;
    loop {
        let packet = match format.next_packet() {
            Ok(packet) => packet,
            Err(symphonia::core::errors::Error::IoError(error))
                if error.kind() == io::ErrorKind::UnexpectedEof =>
            {
                break
            }
            Err(error) => return Err(format!("failed to read audio packet: {error}")),
        };
        if packet.track_id() != track_id {
            continue;
        }
        let decoded = match decoder.decode(&packet) {
            Ok(decoded) => decoded,
            Err(symphonia::core::errors::Error::DecodeError(_)) => continue,
            Err(error) => return Err(format!("failed to decode audio: {error}")),
        };
        let spec = *decoded.spec();
        let decoded_channels = spec.channels.count();
        if spec.rate != sample_rate || decoded_channels != channels {
            return Err("audio format changes within the stream are unsupported".to_owned());
        }
        if decoded.capacity() > MAX_AUDIO_PACKET_FRAMES {
            return Err(format!(
                "decoded audio packet exceeds the {MAX_AUDIO_PACKET_FRAMES}-frame memory limit"
            ));
        }
        let mut samples = SampleBuffer::<f32>::new(decoded.capacity() as u64, spec);
        samples.copy_interleaved_ref(decoded);
        for frame in samples.samples().chunks(channels) {
            decoded_frames = decoded_frames
                .checked_add(1)
                .ok_or_else(|| "decoded audio frame count overflowed".to_owned())?;
            if decoded_frames > max_frames {
                return Err(format!(
                    "audio duration exceeds the {MAX_AUDIO_DURATION_SECONDS}-second limit"
                ));
            }
            let sample = frame.iter().copied().sum::<f32>() as f64 / frame.len().max(1) as f64;
            peak = peak.max(sample.abs());
            square_sum += sample * sample;
            broad_sum += sample * sample;
            low_pass = low_alpha * low_pass + (1.0 - low_alpha) * sample;
            mid_pass = mid_alpha * mid_pass + (1.0 - mid_alpha) * sample;
            low_sum += low_pass * low_pass;
            let middle = mid_pass - low_pass;
            mid_sum += middle * middle;
            let upper = sample - mid_pass;
            high_sum += upper * upper;
            window_len += 1;
            if window_len == window {
                push_audio_window(
                    &mut energy,
                    &mut low,
                    &mut mid,
                    &mut high,
                    broad_sum,
                    low_sum,
                    mid_sum,
                    high_sum,
                    window_len,
                    max_analysis_points,
                )?;
                broad_sum = 0.0;
                low_sum = 0.0;
                mid_sum = 0.0;
                high_sum = 0.0;
                window_len = 0;
            }
        }
    }
    if window_len > 0 {
        push_audio_window(
            &mut energy,
            &mut low,
            &mut mid,
            &mut high,
            broad_sum,
            low_sum,
            mid_sum,
            high_sum,
            window_len,
            max_analysis_points,
        )?;
    }
    let mut onset = Vec::new();
    onset.push(0.0);
    for pair in energy.windows(2) {
        onset.push((pair[1] - pair[0]).max(0.0));
    }
    let tempo = estimate_tempo(&onset, envelope_hz, tempo_min, tempo_max);
    let duration = decoded_frames as f64 / sample_rate as f64;
    let mut beat_grid = Vec::new();
    if let Some(bpm) = tempo.bpm {
        let interval = 60.0 / bpm;
        let mut beat = tempo.beat_offset;
        while beat <= duration {
            beat_grid.push(beat);
            beat += interval;
        }
    }
    Ok(AudioAnalysis {
        path: display_path(path.strip_prefix(cwd).unwrap_or(path)),
        duration,
        channels,
        sample_rate,
        envelope_hz,
        rms: (square_sum / decoded_frames.max(1) as f64).sqrt(),
        peak,
        bpm: tempo.bpm,
        confidence: tempo.confidence,
        tempo_candidates: tempo.candidates,
        tempo_ambiguity: tempo.ambiguity,
        boundary_influence: tempo.boundary_influence,
        beat_offset: tempo.beat_offset,
        beat_grid,
        energy,
        low,
        mid,
        high,
        onset,
    })
}

#[allow(clippy::too_many_arguments)]
fn push_audio_window(
    energy: &mut Vec<f64>,
    low: &mut Vec<f64>,
    mid: &mut Vec<f64>,
    high: &mut Vec<f64>,
    broad_sum: f64,
    low_sum: f64,
    mid_sum: f64,
    high_sum: f64,
    sample_count: usize,
    max_analysis_points: usize,
) -> Result<(), String> {
    ensure_audio_analysis_capacity(energy.len(), max_analysis_points)?;
    let divisor = sample_count.max(1) as f64;
    energy.push((broad_sum / divisor).sqrt());
    low.push((low_sum / divisor).sqrt());
    mid.push((mid_sum / divisor).sqrt());
    high.push((high_sum / divisor).sqrt());
    Ok(())
}

fn audio_frame_limit(
    sample_rate: u32,
    channels: usize,
    declared_frames: Option<u64>,
) -> Result<u64, String> {
    if sample_rate > MAX_AUDIO_SAMPLE_RATE {
        return Err(format!(
            "audio sample rate {sample_rate} Hz exceeds the {MAX_AUDIO_SAMPLE_RATE} Hz limit"
        ));
    }
    if channels > MAX_AUDIO_CHANNELS {
        return Err(format!(
            "audio channel count {channels} exceeds the {MAX_AUDIO_CHANNELS}-channel limit"
        ));
    }
    let max_frames = u64::from(sample_rate)
        .checked_mul(MAX_AUDIO_DURATION_SECONDS)
        .ok_or_else(|| "audio duration limit overflowed".to_owned())?;
    if declared_frames.is_some_and(|frames| frames > max_frames) {
        return Err(format!(
            "audio duration exceeds the {MAX_AUDIO_DURATION_SECONDS}-second limit"
        ));
    }
    Ok(max_frames)
}

fn ensure_audio_analysis_capacity(points: usize, max_points: usize) -> Result<(), String> {
    if points >= max_points {
        Err(format!(
            "audio analysis exceeds the {MAX_AUDIO_ANALYSIS_POINTS}-point memory limit"
        ))
    } else {
        Ok(())
    }
}

fn estimate_tempo(
    onset: &[f64],
    envelope_hz: f64,
    tempo_min: f64,
    tempo_max: f64,
) -> TempoEstimate {
    if onset.len() < 4 || onset.iter().copied().fold(0.0_f64, f64::max) <= 1e-9 {
        return TempoEstimate {
            bpm: None,
            confidence: 0.0,
            beat_offset: 0.0,
            candidates: Vec::new(),
            ambiguity: 0.0,
            boundary_influence: 0.0,
        };
    }
    let min_lag = ((60.0 * envelope_hz / tempo_max).round() as usize).max(1);
    let max_lag = ((60.0 * envelope_hz / tempo_min).round() as usize).min(onset.len() - 1);
    let edge = ((envelope_hz * 0.25).round() as usize).min(onset.len() / 10);
    let mut scores = Vec::new();
    for lag in min_lag..=max_lag {
        let full = normalized_onset_correlation(onset, lag, 0);
        let inner = normalized_onset_correlation(onset, lag, edge);
        let boundary = (full - inner).max(0.0);
        scores.push((lag, (inner * 0.8 + full * 0.2).max(0.0), boundary));
    }
    let mut peaks = scores
        .iter()
        .enumerate()
        .filter(|(index, (_, score, _))| {
            let left = index
                .checked_sub(1)
                .map(|value| scores[value].1)
                .unwrap_or(-1.0);
            let right = scores.get(index + 1).map(|value| value.1).unwrap_or(-1.0);
            *score >= left && *score >= right && *score > 0.0
        })
        .map(|(index, value)| (index, *value))
        .collect::<Vec<_>>();
    peaks.sort_by(|left, right| {
        right
            .1
             .1
            .total_cmp(&left.1 .1)
            .then(left.1 .0.cmp(&right.1 .0))
    });
    peaks.truncate(5);
    let Some((best_index, (best_lag, best_score, best_boundary))) = peaks.first().copied() else {
        return TempoEstimate {
            bpm: None,
            confidence: 0.0,
            beat_offset: 0.0,
            candidates: Vec::new(),
            ambiguity: 0.0,
            boundary_influence: 0.0,
        };
    };
    let primary_bpm = refined_bpm(&scores, best_index, envelope_hz);
    let mut candidates = peaks
        .iter()
        .map(|(index, (_, score, _))| {
            let bpm = refined_bpm(&scores, *index, envelope_hz);
            TempoCandidate {
                bpm,
                score: *score,
                relative_score: (*score / best_score).clamp(0.0, 1.0),
                relation: tempo_relation(bpm, primary_bpm).to_owned(),
            }
        })
        .collect::<Vec<_>>();
    if let Some(primary) = candidates.first_mut() {
        primary.relation = "primary".to_owned();
    }
    let ambiguity = candidates
        .iter()
        .skip(1)
        .map(|candidate| {
            if candidate.relation == "alternative" {
                ((candidate.relative_score - 0.8) / 0.2).clamp(0.0, 1.0)
            } else {
                candidate.relative_score
            }
        })
        .fold(0.0_f64, f64::max);
    let boundary_influence = (best_boundary / best_score.max(1e-9)).clamp(0.0, 1.0);
    let confidence =
        (best_score * (1.0 - 0.55 * ambiguity) * (1.0 - 0.5 * boundary_influence)).clamp(0.0, 1.0);
    let first = onset
        .iter()
        .enumerate()
        .max_by(|left, right| left.1.total_cmp(right.1))
        .map(|(index, _)| index)
        .unwrap_or(0);
    TempoEstimate {
        bpm: Some(primary_bpm),
        confidence,
        beat_offset: (first % best_lag) as f64 / envelope_hz,
        candidates,
        ambiguity,
        boundary_influence,
    }
}

fn normalized_onset_correlation(onset: &[f64], lag: usize, edge: usize) -> f64 {
    let start = lag.saturating_add(edge);
    let end = onset.len().saturating_sub(edge);
    if start >= end {
        return 0.0;
    }
    let mut product = 0.0;
    let mut left_energy = 0.0;
    let mut right_energy = 0.0;
    for index in start..end {
        let left = onset[index - lag];
        let right = onset[index];
        product += left * right;
        left_energy += left * left;
        right_energy += right * right;
    }
    product / (left_energy * right_energy).sqrt().max(1e-12)
}

fn refined_bpm(scores: &[(usize, f64, f64)], index: usize, envelope_hz: f64) -> f64 {
    let (lag, score, _) = scores[index];
    let refined_lag = if index > 0 && index + 1 < scores.len() {
        let left = scores[index - 1].1;
        let right = scores[index + 1].1;
        let denominator = left - 2.0 * score + right;
        let delta = if denominator.abs() > 1e-12 {
            (0.5 * (left - right) / denominator).clamp(-0.5, 0.5)
        } else {
            0.0
        };
        lag as f64 + delta
    } else {
        lag as f64
    };
    60.0 * envelope_hz / refined_lag
}

fn tempo_relation(candidate: f64, primary: f64) -> &'static str {
    let ratio = candidate / primary;
    for (target, name) in [
        (0.5, "half-time"),
        (2.0 / 3.0, "two-thirds"),
        (0.75, "three-quarters"),
        (4.0 / 3.0, "four-thirds"),
        (1.5, "three-halves"),
        (2.0, "double-time"),
    ] {
        if (ratio / target - 1.0).abs() <= 0.04 {
            return name;
        }
    }
    "alternative"
}

fn audio_catalog_lua(analyses: &[AudioAnalysis]) -> String {
    let mut output = String::from("--!tua generated\nreturn {\n");
    for analysis in analyses {
        output.push_str(&format!("  [{}] = {{ path = {}, duration = {:.9}, channels = {}, sampleRate = {}, envelopeHz = {:.6}, rms = {:.9}, peak = {:.9}, bpm = {}, confidence = {:.9}, tempoAmbiguity = {:.9}, boundaryInfluence = {:.9}, beatOffset = {:.9},\n",
            lua_string(&analysis.path), lua_string(&analysis.path), analysis.duration, analysis.channels,
            analysis.sample_rate, analysis.envelope_hz, analysis.rms, analysis.peak,
            analysis.bpm.map(|value| format!("{value:.9}")).unwrap_or_else(|| "nil".to_owned()),
            analysis.confidence, analysis.tempo_ambiguity, analysis.boundary_influence, analysis.beat_offset));
        output.push_str("    tempoCandidates = {");
        for candidate in &analysis.tempo_candidates {
            output.push_str(&format!(
                "{{ bpm = {:.9}, score = {:.9}, relativeScore = {:.9}, relation = {} }},",
                candidate.bpm,
                candidate.score,
                candidate.relative_score,
                lua_string(&candidate.relation)
            ));
        }
        output.push_str("},\n");
        for (name, values) in [
            ("beatGrid", &analysis.beat_grid),
            ("energy", &analysis.energy),
            ("low", &analysis.low),
            ("mid", &analysis.mid),
            ("high", &analysis.high),
            ("onset", &analysis.onset),
        ] {
            output.push_str("    ");
            output.push_str(name);
            output.push_str(" = {");
            for (index, value) in values.iter().enumerate() {
                if index > 0 {
                    output.push(',');
                }
                output.push_str(&format!("{value:.7}"));
            }
            output.push_str("},\n");
        }
        output.push_str("  },\n");
    }
    output.push_str("}\n");
    output
}

fn lua_string(value: &str) -> String {
    format!(
        "\"{}\"",
        value
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('\n', "\\n")
    )
}

pub(crate) fn run_asset_pipelines(cwd: &Path, config: &ProjectConfig) -> AssetPipelineOutcome {
    let mut outcome = AssetPipelineOutcome::default();
    let mut output_roots = Vec::<(String, PathBuf)>::new();
    let mut names = BTreeSet::new();

    for pipeline in &config.asset_pipelines {
        if !names.insert(pipeline.name.clone()) {
            outcome.diagnostics.push(pipeline_error(
                cwd,
                pipeline,
                "pipeline names must be unique",
            ));
            continue;
        }
        match validated_output_root(cwd, config, pipeline) {
            Ok(root) => {
                if let Some((owner, _)) = output_roots
                    .iter()
                    .find(|(_, existing)| root.starts_with(existing) || existing.starts_with(&root))
                {
                    outcome.diagnostics.push(pipeline_error(
                        cwd,
                        pipeline,
                        format!(
                            "output `{}` overlaps asset pipeline `{owner}`; each pipeline must own a separate generated directory",
                            display_path(&pipeline.output)
                        ),
                    ));
                    continue;
                }
                output_roots.push((pipeline.name.clone(), root));
            }
            Err(diagnostic) => {
                outcome.diagnostics.push(diagnostic);
                continue;
            }
        }

        run_one_pipeline(cwd, pipeline, &mut outcome);
    }

    outcome
}

pub(crate) fn asset_pipeline_input_files(
    cwd: &Path,
    config: &ProjectConfig,
) -> Result<Vec<PathBuf>, Diagnostic> {
    let mut files = BTreeSet::new();

    for pipeline in &config.asset_pipelines {
        for pattern in &pipeline.inputs {
            let paths = matching_input_files(cwd, pattern).map_err(|message| {
                pipeline_error(cwd, pipeline, format!("input `{pattern}`: {message}"))
            })?;
            files.extend(paths);
        }
    }

    Ok(files.into_iter().collect())
}

pub(crate) fn asset_pipeline_watch_roots(cwd: &Path, config: &ProjectConfig) -> Vec<PathBuf> {
    let mut roots = BTreeSet::new();

    for pipeline in &config.asset_pipelines {
        for input in &pipeline.inputs {
            if let Ok(relative) = safe_relative_pattern(input) {
                let text = display_path(&relative);
                let wildcard = text.contains('*') || text.contains('?');
                let absolute = cwd.join(&relative);
                roots.insert(if !wildcard && absolute.is_dir() {
                    absolute
                } else if !wildcard {
                    absolute.parent().unwrap_or(cwd).to_path_buf()
                } else {
                    cwd.join(pattern_anchor(&relative))
                });
            }
        }
    }

    roots.into_iter().collect()
}

fn run_one_pipeline(
    cwd: &Path,
    pipeline: &AssetPipelineConfig,
    outcome: &mut AssetPipelineOutcome,
) {
    if !valid_pipeline_name(&pipeline.name) {
        outcome.diagnostics.push(pipeline_error(
            cwd,
            pipeline,
            "name must contain only letters, digits, `.`, `-`, or `_`",
        ));
        return;
    }
    if pipeline.builtin.is_some() && !pipeline.command.is_empty() {
        outcome.diagnostics.push(pipeline_error(
            cwd,
            pipeline,
            "`builtin` and `command` are mutually exclusive",
        ));
        return;
    }
    if pipeline.command.is_empty() && pipeline.builtin.is_none() {
        outcome
            .diagnostics
            .push(pipeline_error(cwd, pipeline, "command must not be empty"));
        return;
    }
    if let Err(message) = validate_resource_limits(pipeline) {
        outcome
            .diagnostics
            .push(pipeline_error(cwd, pipeline, message));
        return;
    }

    let input_files = match pipeline_inputs(cwd, pipeline) {
        Ok(files) => files,
        Err(diagnostics) => {
            outcome.diagnostics.extend(diagnostics);
            return;
        }
    };
    let cache_key = match pipeline_cache_key(cwd, pipeline, &input_files) {
        Ok(key) => key,
        Err(diagnostic) => {
            outcome.diagnostics.push(diagnostic);
            return;
        }
    };
    let pipeline_cache = cwd
        .join(".tua")
        .join("cache")
        .join("asset-pipelines")
        .join(&pipeline.name);
    let cache_entry = pipeline_cache.join(&cache_key);

    let manifest = match read_valid_cached_manifest(&cache_entry, pipeline.max_manifest_bytes) {
        Ok(Some(manifest)) => {
            outcome.cached += 1;
            manifest
        }
        Ok(None) => {
            if let Err(diagnostic) = clear_cache_entry(&cache_entry) {
                outcome.diagnostics.push(diagnostic);
                return;
            }
            match execute_importer(cwd, pipeline, &input_files, &cache_key, &cache_entry) {
                Ok(manifest) => {
                    outcome.executed += 1;
                    manifest
                }
                Err(mut diagnostics) => {
                    outcome.diagnostics.append(&mut diagnostics);
                    return;
                }
            }
        }
        Err(diagnostic) => {
            if let Err(clear_diagnostic) = clear_cache_entry(&cache_entry) {
                outcome.diagnostics.push(diagnostic);
                outcome.diagnostics.push(clear_diagnostic);
                return;
            }
            match execute_importer(cwd, pipeline, &input_files, &cache_key, &cache_entry) {
                Ok(manifest) => {
                    outcome.executed += 1;
                    manifest
                }
                Err(mut diagnostics) => {
                    outcome.diagnostics.append(&mut diagnostics);
                    return;
                }
            }
        }
    };

    outcome
        .diagnostics
        .extend(manifest_diagnostics(cwd, pipeline, &manifest));

    if let Err(mut diagnostics) = materialize_outputs(cwd, pipeline, &cache_entry, &manifest) {
        outcome.diagnostics.append(&mut diagnostics);
    }
}

fn pipeline_inputs(
    cwd: &Path,
    pipeline: &AssetPipelineConfig,
) -> Result<Vec<PathBuf>, Vec<Diagnostic>> {
    let mut files = BTreeSet::new();
    let mut diagnostics = Vec::new();

    if pipeline.inputs.is_empty() {
        diagnostics.push(pipeline_error(
            cwd,
            pipeline,
            "inputs must contain at least one file, directory, or glob",
        ));
    }

    for pattern in &pipeline.inputs {
        match matching_input_files(cwd, pattern) {
            Ok(matches) if matches.is_empty() => diagnostics.push(pipeline_error(
                cwd,
                pipeline,
                format!("input pattern `{pattern}` matched no files"),
            )),
            Ok(matches) => files.extend(matches),
            Err(message) => diagnostics.push(pipeline_error(
                cwd,
                pipeline,
                format!("input `{pattern}`: {message}"),
            )),
        }
    }

    if diagnostics.is_empty() {
        Ok(files.into_iter().collect())
    } else {
        Err(diagnostics)
    }
}

fn pipeline_cache_key(
    cwd: &Path,
    pipeline: &AssetPipelineConfig,
    inputs: &[PathBuf],
) -> Result<String, Diagnostic> {
    let mut hash = StableHash::new();
    hash.add(b"tua-asset-pipeline-v1\0");
    hash.add(pipeline.name.as_bytes());
    hash.add(b"\0");
    hash.add(pipeline.version.as_bytes());
    hash.add(b"\0");
    hash.add(display_path(&pipeline.output).as_bytes());
    hash.add(b"\0limits\0");
    hash.add(pipeline.timeout_ms.to_string().as_bytes());
    hash.add(b"\0");
    hash.add(pipeline.max_manifest_bytes.to_string().as_bytes());
    hash.add(b"\0");
    hash.add(pipeline.max_stderr_bytes.to_string().as_bytes());

    if let Some(builtin) = &pipeline.builtin {
        hash.add(b"\0builtin\0");
        hash.add(builtin.as_bytes());
        if builtin == "audio-analysis" {
            hash.add(b"\0analyzer-version\0");
            hash.add(AUDIO_ANALYZER_VERSION.as_bytes());
        }
    }
    for (name, value) in &pipeline.options {
        hash.add(b"\0option\0");
        hash.add(name.as_bytes());
        hash.add(value.to_string().as_bytes());
    }

    for argument in &pipeline.command {
        hash.add(b"\0command\0");
        hash.add(argument.as_bytes());
    }

    if let Some(program) = pipeline.command.first() {
        let program_path = cwd.join(program);
        if program_path.is_file() {
            hash_file_into(&program_path, &mut hash)
                .map_err(|source| io_diagnostic(&program_path, source))?;
        }
    }

    for path in inputs {
        hash.add(b"\0input\0");
        hash.add(
            path.strip_prefix(cwd)
                .unwrap_or(path)
                .to_string_lossy()
                .replace('\\', "/")
                .as_bytes(),
        );
        hash_file_into(path, &mut hash).map_err(|source| io_diagnostic(path, source))?;
    }

    Ok(hash.finish())
}

fn execute_importer(
    cwd: &Path,
    pipeline: &AssetPipelineConfig,
    inputs: &[PathBuf],
    cache_key: &str,
    cache_entry: &Path,
) -> Result<ImportManifest, Vec<Diagnostic>> {
    let parent = cache_entry
        .parent()
        .expect("cache entry should have a parent");
    if let Err(source) = fs::create_dir_all(parent) {
        return Err(vec![io_diagnostic(parent, source)]);
    }

    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos());
    let staging = parent.join(format!(".{cache_key}.tmp-{}-{nonce}", std::process::id()));
    let files_dir = staging.join("files");
    let manifest_path = staging.join("manifest.json");
    if let Err(source) = fs::create_dir_all(&files_dir) {
        return Err(vec![io_diagnostic(&files_dir, source)]);
    }

    let input_strings = inputs
        .iter()
        .map(|path| path.to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    let manifest_text = manifest_path.to_string_lossy().into_owned();
    let output_text = files_dir.to_string_lossy().into_owned();
    let project_text = cwd.to_string_lossy().into_owned();
    let mut arguments = Vec::new();

    for argument in pipeline.command.iter().skip(1) {
        if argument == "{inputs}" {
            arguments.extend(input_strings.iter().cloned());
        } else {
            arguments.push(
                argument
                    .replace("{manifest}", &manifest_text)
                    .replace("{output}", &output_text)
                    .replace("{project}", &project_text)
                    .replace("{cache_key}", cache_key),
            );
        }
    }

    if let Some(builtin) = pipeline.builtin.as_deref() {
        let result = match builtin {
            "audio-analysis" => execute_audio_analysis(cwd, pipeline, inputs, &files_dir),
            _ => Err(vec![pipeline_error(
                cwd,
                pipeline,
                format!("unknown built-in importer `{builtin}`"),
            )]),
        };
        let manifest = match result {
            Ok(manifest) => manifest,
            Err(diagnostics) => {
                let _ = fs::remove_dir_all(&staging);
                return Err(diagnostics);
            }
        };
        if let Err(source) = fs::write(
            &manifest_path,
            serde_json::to_vec_pretty(&manifest).unwrap_or_default(),
        ) {
            let _ = fs::remove_dir_all(&staging);
            return Err(vec![io_diagnostic(&manifest_path, source)]);
        }
        if let Err(diagnostic) = validate_manifest(cwd, pipeline, &staging, &manifest) {
            let _ = fs::remove_dir_all(&staging);
            return Err(vec![diagnostic]);
        }
        if cache_entry.exists() {
            let _ = fs::remove_dir_all(&staging);
            return read_manifest(
                &cache_entry.join("manifest.json"),
                pipeline.max_manifest_bytes,
            )
            .map_err(|error| vec![error]);
        }
        if let Err(source) = fs::rename(&staging, cache_entry) {
            let diagnostic = io_diagnostic(cache_entry, source);
            let _ = fs::remove_dir_all(&staging);
            return Err(vec![diagnostic]);
        }
        return Ok(manifest);
    }

    let configured_program = &pipeline.command[0];
    let local_program = cwd.join(configured_program);
    let program = if configured_program.contains('/') || configured_program.contains('\\') {
        local_program
    } else {
        PathBuf::from(configured_program)
    };
    let mut command = Command::new(&program);
    command
        .args(arguments)
        .current_dir(cwd)
        .env("TUA_ASSET_PIPELINE_NAME", &pipeline.name)
        .env("TUA_ASSET_PIPELINE_PROJECT_ROOT", cwd)
        .env("TUA_ASSET_PIPELINE_OUTPUT_DIR", &files_dir)
        .env("TUA_ASSET_PIPELINE_MANIFEST", &manifest_path)
        .env("TUA_ASSET_PIPELINE_CACHE_KEY", cache_key)
        .env(
            "TUA_ASSET_PIPELINE_INPUTS",
            serde_json::to_string(&input_strings).unwrap_or_else(|_| "[]".to_owned()),
        );
    let command_output = match run_importer_command(
        &mut command,
        Duration::from_millis(pipeline.timeout_ms),
        pipeline.max_stderr_bytes as usize,
    ) {
        Ok(output) => output,
        Err(ImporterCommandError::Start(source)) => {
            let _ = fs::remove_dir_all(&staging);
            return Err(vec![pipeline_error(
                cwd,
                pipeline,
                format!("failed to start `{}`: {source}", display_path(&program)),
            )]);
        }
        Err(ImporterCommandError::TimedOut) => {
            let _ = fs::remove_dir_all(&staging);
            return Err(vec![pipeline_error(
                cwd,
                pipeline,
                format!("importer timed out after {} ms", pipeline.timeout_ms),
            )]);
        }
    };

    let mut diagnostics = match read_manifest(&manifest_path, pipeline.max_manifest_bytes) {
        Ok(manifest) => manifest_diagnostics(cwd, pipeline, &manifest),
        Err(_) => Vec::new(),
    };

    if !command_output.status.success() {
        let mut stderr = String::from_utf8_lossy(&command_output.stderr)
            .trim()
            .to_owned();
        if command_output.stderr_truncated {
            stderr.push_str("… [stderr truncated]");
        }
        let suffix = if stderr.is_empty() {
            String::new()
        } else {
            format!(": {stderr}")
        };
        diagnostics.push(pipeline_error(
            cwd,
            pipeline,
            format!("importer exited with {}{suffix}", command_output.status),
        ));
        let _ = fs::remove_dir_all(&staging);
        return Err(diagnostics);
    }

    let manifest = match read_manifest(&manifest_path, pipeline.max_manifest_bytes) {
        Ok(manifest) => manifest,
        Err(diagnostic) => {
            let _ = fs::remove_dir_all(&staging);
            return Err(vec![diagnostic]);
        }
    };
    if let Err(diagnostic) = validate_manifest(cwd, pipeline, &staging, &manifest) {
        let _ = fs::remove_dir_all(&staging);
        return Err(vec![diagnostic]);
    }

    if cache_entry.exists() {
        let _ = fs::remove_dir_all(&staging);
        return read_manifest(
            &cache_entry.join("manifest.json"),
            pipeline.max_manifest_bytes,
        )
        .map_err(|error| vec![error]);
    }
    if let Err(source) = fs::rename(&staging, cache_entry) {
        let diagnostic = io_diagnostic(cache_entry, source);
        let _ = fs::remove_dir_all(&staging);
        return Err(vec![diagnostic]);
    }

    Ok(manifest)
}

struct ImporterCommandOutput {
    status: ExitStatus,
    stderr: Vec<u8>,
    stderr_truncated: bool,
}

enum ImporterCommandError {
    Start(io::Error),
    TimedOut,
}

fn run_importer_command(
    command: &mut Command,
    timeout: Duration,
    max_stderr_bytes: usize,
) -> Result<ImporterCommandOutput, ImporterCommandError> {
    let mut child = command
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(ImporterCommandError::Start)?;
    let stderr = child
        .stderr
        .take()
        .expect("piped importer stderr should be available");
    let (stderr_tx, stderr_rx) = mpsc::sync_channel(1);
    thread::spawn(move || {
        let _ = stderr_tx.send(read_bounded(stderr, max_stderr_bytes));
    });

    let deadline = Instant::now() + timeout;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if Instant::now() >= deadline => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(ImporterCommandError::TimedOut);
            }
            Ok(None) => thread::sleep(Duration::from_millis(10)),
            Err(source) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(ImporterCommandError::Start(source));
            }
        }
    };
    let (stderr, stderr_truncated) = stderr_rx
        .recv_timeout(Duration::from_secs(1))
        .unwrap_or_else(|_| (Vec::new(), true));
    Ok(ImporterCommandOutput {
        status,
        stderr,
        stderr_truncated,
    })
}

fn read_bounded(mut reader: impl Read, limit: usize) -> (Vec<u8>, bool) {
    let mut captured = Vec::with_capacity(limit.min(8 * 1024));
    let mut truncated = false;
    let mut buffer = [0_u8; 8 * 1024];
    loop {
        let read = match reader.read(&mut buffer) {
            Ok(0) | Err(_) => break,
            Ok(read) => read,
        };
        let remaining = limit.saturating_sub(captured.len());
        let copied = remaining.min(read);
        captured.extend_from_slice(&buffer[..copied]);
        truncated |= copied < read;
    }
    (captured, truncated)
}

fn read_valid_cached_manifest(
    cache_entry: &Path,
    max_manifest_bytes: u64,
) -> Result<Option<ImportManifest>, Diagnostic> {
    if !cache_entry.is_dir() {
        return Ok(None);
    }
    let manifest = read_manifest(&cache_entry.join("manifest.json"), max_manifest_bytes)?;
    if manifest.version != MANIFEST_VERSION {
        return Ok(None);
    }
    for output in &manifest.outputs {
        let Ok(relative) = safe_relative_path(output) else {
            return Ok(None);
        };
        let path = cache_entry.join("files").join(relative);
        if !fs::symlink_metadata(path)
            .is_ok_and(|metadata| metadata.is_file() && !metadata.file_type().is_symlink())
        {
            return Ok(None);
        }
    }
    Ok(Some(manifest))
}

fn clear_cache_entry(cache_entry: &Path) -> Result<(), Diagnostic> {
    match fs::symlink_metadata(cache_entry) {
        Ok(metadata) if metadata.file_type().is_symlink() || metadata.is_file() => {
            fs::remove_file(cache_entry).map_err(|source| io_diagnostic(cache_entry, source))
        }
        Ok(metadata) if metadata.is_dir() => {
            fs::remove_dir_all(cache_entry).map_err(|source| io_diagnostic(cache_entry, source))
        }
        Ok(_) => Ok(()),
        Err(source) if source.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(source) => Err(io_diagnostic(cache_entry, source)),
    }
}

fn read_manifest(path: &Path, max_bytes: u64) -> Result<ImportManifest, Diagnostic> {
    let file = File::open(path).map_err(|source| io_diagnostic(path, source))?;
    let mut source = String::new();
    file.take(max_bytes.saturating_add(1))
        .read_to_string(&mut source)
        .map_err(|source| io_diagnostic(path, source))?;
    if source.len() as u64 > max_bytes {
        return Err(Diagnostic::error(
            path,
            1,
            1,
            format!("asset pipeline manifest exceeds the {max_bytes}-byte limit"),
        )
        .with_code(DiagnosticCode::ASSET_PIPELINE));
    }
    serde_json::from_str(&source).map_err(|source| {
        Diagnostic::error(path, source.line(), source.column(), source.to_string())
            .with_code(DiagnosticCode::ASSET_PIPELINE)
    })
}

fn validate_manifest(
    cwd: &Path,
    pipeline: &AssetPipelineConfig,
    staging: &Path,
    manifest: &ImportManifest,
) -> Result<(), Diagnostic> {
    if manifest.version != MANIFEST_VERSION {
        return Err(pipeline_error(
            cwd,
            pipeline,
            format!(
                "manifest version {} is unsupported; expected {MANIFEST_VERSION}",
                manifest.version
            ),
        ));
    }

    let mut outputs = BTreeSet::new();
    for output in &manifest.outputs {
        let relative = safe_relative_path(output).map_err(|message| {
            pipeline_error(
                cwd,
                pipeline,
                format!("manifest output `{output}` {message}"),
            )
        })?;
        if !outputs.insert(relative.clone()) {
            return Err(pipeline_error(
                cwd,
                pipeline,
                format!("manifest output `{output}` is duplicated"),
            ));
        }
        let generated_path = staging.join("files").join(&relative);
        let generated_is_regular_file = fs::symlink_metadata(&generated_path)
            .is_ok_and(|metadata| metadata.is_file() && !metadata.file_type().is_symlink());
        if !generated_is_regular_file {
            return Err(pipeline_error(
                cwd,
                pipeline,
                format!("manifest output `{output}` was not written by the importer"),
            ));
        }
    }
    Ok(())
}

fn materialize_outputs(
    cwd: &Path,
    pipeline: &AssetPipelineConfig,
    cache_entry: &Path,
    manifest: &ImportManifest,
) -> Result<(), Vec<Diagnostic>> {
    let output_root = cwd.join(&pipeline.output);
    let state_path = cache_entry
        .parent()
        .expect("cache entry should have parent")
        .join("materialized.json");
    let previous = fs::read_to_string(&state_path)
        .ok()
        .and_then(|source| serde_json::from_str::<MaterializedState>(&source).ok())
        .unwrap_or_default();
    let current = manifest.outputs.iter().cloned().collect::<BTreeSet<_>>();
    let previously_owned = previous.outputs.iter().cloned().collect::<BTreeSet<_>>();
    let mut diagnostics = Vec::new();

    for stale in previous
        .outputs
        .iter()
        .filter(|output| !current.contains(*output))
    {
        let Ok(relative) = safe_relative_path(stale) else {
            continue;
        };
        let path = output_root.join(relative);
        match fs::remove_file(&path) {
            Ok(()) => {}
            Err(source) if source.kind() == io::ErrorKind::NotFound => {}
            Err(source) => diagnostics.push(io_diagnostic(&path, source)),
        }
    }

    for output in &manifest.outputs {
        let relative = match safe_relative_path(output) {
            Ok(path) => path,
            Err(message) => {
                diagnostics.push(pipeline_error(
                    cwd,
                    pipeline,
                    format!("manifest output `{output}` {message}"),
                ));
                continue;
            }
        };
        let source = cache_entry.join("files").join(&relative);
        let destination = output_root.join(&relative);
        if let Err(message) = reject_symlink_path(cwd, &destination) {
            diagnostics.push(pipeline_error(cwd, pipeline, message));
            continue;
        }
        if destination.is_file() && !previously_owned.contains(output) {
            let same_content = fs::read(&source)
                .ok()
                .zip(fs::read(&destination).ok())
                .is_some_and(|(left, right)| generated_content_matches(&left, &right));
            if !same_content {
                diagnostics.push(pipeline_error(
                    cwd,
                    pipeline,
                    format!(
                        "generated output `{}` would overwrite an unowned source file",
                        display_path(&destination)
                    ),
                ));
                continue;
            }
        }
        if let Some(parent) = destination.parent() {
            if let Err(source) = fs::create_dir_all(parent) {
                diagnostics.push(io_diagnostic(parent, source));
                continue;
            }
        }
        if let Err(source) = fs::copy(&source, &destination) {
            diagnostics.push(io_diagnostic(&destination, source));
        }
    }

    if diagnostics.is_empty() {
        let state = MaterializedState {
            outputs: manifest.outputs.clone(),
        };
        if let Some(parent) = state_path.parent() {
            if let Err(source) = fs::create_dir_all(parent) {
                return Err(vec![io_diagnostic(parent, source)]);
            }
        }
        let source = serde_json::to_string_pretty(&state).expect("state should serialize");
        if let Err(error) = fs::write(&state_path, format!("{source}\n")) {
            diagnostics.push(io_diagnostic(&state_path, error));
        }
    }

    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

fn generated_content_matches(generated: &[u8], existing: &[u8]) -> bool {
    generated == existing
        || std::str::from_utf8(generated)
            .ok()
            .zip(std::str::from_utf8(existing).ok())
            .is_some_and(|(generated, existing)| {
                generated.replace("\r\n", "\n") == existing.replace("\r\n", "\n")
            })
}

fn manifest_diagnostics(
    cwd: &Path,
    pipeline: &AssetPipelineConfig,
    manifest: &ImportManifest,
) -> Vec<Diagnostic> {
    manifest
        .diagnostics
        .iter()
        .map(|item| {
            let severity = match item.severity.as_str() {
                "error" => DiagnosticSeverity::Error,
                "warning" => DiagnosticSeverity::Warning,
                "info" => DiagnosticSeverity::Info,
                "hint" => DiagnosticSeverity::Hint,
                _ => DiagnosticSeverity::Error,
            };
            let path = safe_relative_path(&item.path)
                .ok()
                .map(|path| cwd.join(path))
                .unwrap_or_else(|| cwd.join("tua.toml"));
            let mut diagnostic = Diagnostic::new(
                path,
                item.line,
                item.column,
                severity,
                format!("asset pipeline `{}`: {}", pipeline.name, item.message),
            )
            .with_code(
                item.code
                    .and_then(DiagnosticCode::new)
                    .unwrap_or(DiagnosticCode::ASSET_PIPELINE),
            );
            if let (Some(end_line), Some(end_column)) = (item.end_line, item.end_column) {
                diagnostic = diagnostic.with_range(item.line, item.column, end_line, end_column);
            }
            diagnostic
        })
        .collect()
}

fn validated_output_root(
    cwd: &Path,
    config: &ProjectConfig,
    pipeline: &AssetPipelineConfig,
) -> Result<PathBuf, Diagnostic> {
    let output = safe_relative_path(&pipeline.output.to_string_lossy()).map_err(|message| {
        pipeline_error(
            cwd,
            pipeline,
            format!("output `{}` {message}", display_path(&pipeline.output)),
        )
    })?;
    let source = safe_relative_path(&config.source.to_string_lossy())
        .map_err(|message| pipeline_error(cwd, pipeline, format!("source root {message}")))?;
    let output = cwd.join(output);
    let source = cwd.join(source);
    if !output.starts_with(&source) || output == source {
        return Err(pipeline_error(
            cwd,
            pipeline,
            format!(
                "output `{}` must be a dedicated directory below source root `{}`",
                display_path(&pipeline.output),
                display_path(&config.source)
            ),
        ));
    }
    Ok(output)
}

fn matching_input_files(cwd: &Path, pattern: &str) -> Result<Vec<PathBuf>, String> {
    let relative = safe_relative_pattern(pattern)?;
    let pattern_text = display_path(&relative);
    let has_wildcard = pattern_text.contains('*') || pattern_text.contains('?');
    let absolute = cwd.join(&relative);
    let mut files = Vec::new();

    if !has_wildcard {
        if absolute.is_file() {
            return Ok(vec![absolute]);
        }
        if absolute.is_dir() {
            discover_regular_files(&absolute, &mut files).map_err(|error| error.to_string())?;
            files.sort();
            return Ok(files);
        }
        return Ok(Vec::new());
    }

    let anchor = cwd.join(pattern_anchor(&relative));
    if !anchor.exists() {
        return Ok(Vec::new());
    }
    discover_regular_files(&anchor, &mut files).map_err(|error| error.to_string())?;
    files.retain(|path| {
        let candidate = path
            .strip_prefix(cwd)
            .unwrap_or(path)
            .to_string_lossy()
            .replace('\\', "/");
        glob_matches(&pattern_text, &candidate)
    });
    files.sort();
    Ok(files)
}

fn discover_regular_files(dir: &Path, files: &mut Vec<PathBuf>) -> io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            if path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| matches!(name, ".git" | ".hg" | ".svn"))
            {
                continue;
            }
            discover_regular_files(&path, files)?;
        } else if file_type.is_file() {
            files.push(path);
        }
    }
    Ok(())
}

fn glob_matches(pattern: &str, value: &str) -> bool {
    fn matches_from(
        pattern: &[u8],
        value: &[u8],
        pi: usize,
        vi: usize,
        memo: &mut BTreeMap<(usize, usize), bool>,
    ) -> bool {
        if let Some(result) = memo.get(&(pi, vi)) {
            return *result;
        }
        let result = if pi == pattern.len() {
            vi == value.len()
        } else if pattern[pi] == b'*' {
            let double = pattern.get(pi + 1) == Some(&b'*');
            let next = pi + if double { 2 } else { 1 };
            matches_from(pattern, value, next, vi, memo)
                || (double
                    && pattern.get(next) == Some(&b'/')
                    && matches_from(pattern, value, next + 1, vi, memo))
                || (vi < value.len()
                    && (double || value[vi] != b'/')
                    && matches_from(pattern, value, pi, vi + 1, memo))
        } else if vi < value.len()
            && (pattern[pi] == b'?' && value[vi] != b'/' || pattern[pi] == value[vi])
        {
            matches_from(pattern, value, pi + 1, vi + 1, memo)
        } else {
            false
        };
        memo.insert((pi, vi), result);
        result
    }

    matches_from(
        pattern.as_bytes(),
        value.as_bytes(),
        0,
        0,
        &mut BTreeMap::new(),
    )
}

fn pattern_anchor(pattern: &Path) -> PathBuf {
    let mut anchor = PathBuf::new();
    for component in pattern.components() {
        let text = component.as_os_str().to_string_lossy();
        if text.contains('*') || text.contains('?') {
            break;
        }
        anchor.push(component.as_os_str());
    }
    if anchor.as_os_str().is_empty() {
        PathBuf::from(".")
    } else if pattern == anchor {
        anchor.parent().unwrap_or(Path::new(".")).to_path_buf()
    } else {
        anchor
    }
}

fn safe_relative_pattern(value: &str) -> Result<PathBuf, String> {
    safe_relative(value, true)
}

fn safe_relative_path(value: &str) -> Result<PathBuf, String> {
    safe_relative(value, false)
}

fn safe_relative(value: &str, allow_wildcards: bool) -> Result<PathBuf, String> {
    if value.trim().is_empty() {
        return Err("must not be empty".to_owned());
    }
    let path = Path::new(value);
    if path.is_absolute() {
        return Err("must be relative to the project root".to_owned());
    }
    let mut result = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Normal(value) => {
                let text = value.to_string_lossy();
                if !allow_wildcards && (text.contains('*') || text.contains('?')) {
                    return Err("must not contain wildcards".to_owned());
                }
                result.push(value);
            }
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err("must not escape the project root".to_owned());
            }
        }
    }
    if result.as_os_str().is_empty() {
        Err("must not resolve to the project root".to_owned())
    } else {
        Ok(result)
    }
}

fn hash_file_into(path: &Path, hash: &mut StableHash) -> io::Result<()> {
    let mut file = File::open(path)?;
    let mut buffer = [0u8; 16 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            return Ok(());
        }
        hash.add(&buffer[..read]);
    }
}

struct StableHash {
    left: u64,
    right: u64,
}

impl StableHash {
    fn new() -> Self {
        Self {
            left: FNV_OFFSET_BASIS,
            right: FNV_OFFSET_BASIS ^ 0x9e3779b97f4a7c15,
        }
    }

    fn add(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.left ^= u64::from(*byte);
            self.left = self.left.wrapping_mul(FNV_PRIME);
            self.right ^= u64::from(*byte).rotate_left(1);
            self.right = self.right.wrapping_mul(FNV_PRIME ^ 0x100);
        }
    }

    fn finish(&self) -> String {
        format!("{:016x}{:016x}", self.left, self.right)
    }
}

fn valid_pipeline_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'))
}

fn validate_resource_limits(pipeline: &AssetPipelineConfig) -> Result<(), String> {
    if pipeline.timeout_ms == 0 || pipeline.timeout_ms > MAX_IMPORTER_TIMEOUT_MS {
        return Err(format!(
            "timeout_ms must be between 1 and {MAX_IMPORTER_TIMEOUT_MS}"
        ));
    }
    if pipeline.max_manifest_bytes == 0 || pipeline.max_manifest_bytes > MAX_MANIFEST_BYTES {
        return Err(format!(
            "max_manifest_bytes must be between 1 and {MAX_MANIFEST_BYTES}"
        ));
    }
    if pipeline.max_stderr_bytes > MAX_STDERR_BYTES {
        return Err(format!(
            "max_stderr_bytes must not exceed {MAX_STDERR_BYTES}"
        ));
    }
    Ok(())
}

fn pipeline_error(
    cwd: &Path,
    pipeline: &AssetPipelineConfig,
    message: impl Into<String>,
) -> Diagnostic {
    Diagnostic::error(
        cwd.join("tua.toml"),
        1,
        1,
        format!("asset pipeline `{}`: {}", pipeline.name, message.into()),
    )
    .with_code(DiagnosticCode::ASSET_PIPELINE)
}

fn display_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn glob_matching_supports_recursive_and_component_wildcards() {
        assert!(glob_matches("maps/**/*.json", "maps/world/level.json"));
        assert!(glob_matches("maps/**/*.json", "maps/level.json"));
        assert!(glob_matches("assets/?.png", "assets/a.png"));
        assert!(!glob_matches("assets/*.png", "assets/ui/a.png"));
    }

    #[test]
    fn generated_paths_cannot_escape_their_owned_root() {
        assert!(safe_relative_path("types/game.d.tua").is_ok());
        assert!(safe_relative_path("../main.tua").is_err());
        assert!(safe_relative_path("/tmp/main.tua").is_err());
    }

    #[test]
    fn generated_text_matches_across_checkout_line_endings() {
        assert!(generated_content_matches(
            b"first\nsecond\n",
            b"first\r\nsecond\r\n"
        ));
        assert!(!generated_content_matches(
            b"first\nsecond\n",
            b"first\r\nchanged\r\n"
        ));
        assert!(!generated_content_matches(
            &[0xff, b'\n'],
            &[0xff, b'\r', b'\n']
        ));
    }

    #[test]
    fn bounded_reads_keep_only_the_configured_prefix() {
        let (captured, truncated) = read_bounded(io::Cursor::new(b"abcdefgh"), 4);
        assert_eq!(captured, b"abcd");
        assert!(truncated);
    }

    #[test]
    fn audio_limits_reject_excessive_format_and_analysis_sizes() {
        assert!(audio_frame_limit(MAX_AUDIO_SAMPLE_RATE + 1, 1, None)
            .unwrap_err()
            .contains("sample rate"));
        assert!(audio_frame_limit(48_000, MAX_AUDIO_CHANNELS + 1, None)
            .unwrap_err()
            .contains("channel count"));
        assert!(
            audio_frame_limit(48_000, 2, Some(48_000 * (MAX_AUDIO_DURATION_SECONDS + 1)))
                .unwrap_err()
                .contains("duration")
        );
        assert!(ensure_audio_analysis_capacity(
            MAX_AUDIO_ANALYSIS_POINTS,
            MAX_AUDIO_ANALYSIS_POINTS
        )
        .is_err());
    }

    #[test]
    fn tempo_estimation_reports_metrical_candidates_and_calibrates_confidence() {
        let mut onset = vec![0.0; 160];
        for index in (0..160).step_by(10) {
            onset[index] = 1.0;
        }
        let estimate = estimate_tempo(&onset, 20.0, 60.0, 200.0);

        assert_eq!(estimate.bpm.map(f64::round), Some(120.0));
        assert!(estimate.candidates.len() >= 2);
        assert!(estimate
            .candidates
            .iter()
            .any(|candidate| candidate.relation == "half-time"));
        assert!(estimate.ambiguity > 0.8);
        assert!(estimate.confidence < 0.6);
    }

    #[test]
    fn tempo_estimation_penalizes_edge_only_correlations() {
        let mut onset = vec![0.0; 100];
        onset[0] = 1.0;
        onset[10] = 1.0;
        let estimate = estimate_tempo(&onset, 20.0, 60.0, 200.0);

        assert!(estimate.boundary_influence > 0.5);
        assert!(estimate.confidence < 0.2);
    }
}
