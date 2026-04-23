use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, Sample, SampleFormat, SizedSample, Stream};
use std::io::{self, Write};
#[cfg(target_os = "linux")]
use std::process::Command;
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::time::Duration;

pub fn audio_listener() -> Result<(), Box<dyn std::error::Error>> {
    let host = cpal::default_host();
    let device = pick_capture_device(&host)?;
    let device_name = device.name()?;

    let supported_config = device.default_input_config()?;
    let sample_format = supported_config.sample_format();
    let config = supported_config.config();
    let channels = usize::from(config.channels);

    let (tx, rx) = mpsc::channel::<f32>();

    println!("Using capture source: {device_name}");
    println!(
        "Format: {:?}, {} channel(s) @ {}Hz",
        sample_format, config.channels, config.sample_rate.0
    );

    let stream = match sample_format {
        SampleFormat::F32 => build_input_stream::<f32>(&device, &config, channels, tx)?,
        SampleFormat::I8 => build_input_stream::<i8>(&device, &config, channels, tx)?,
        SampleFormat::I16 => build_input_stream::<i16>(&device, &config, channels, tx)?,
        SampleFormat::I24 => build_input_stream::<cpal::I24>(&device, &config, channels, tx)?,
        SampleFormat::I32 => build_input_stream::<i32>(&device, &config, channels, tx)?,
        SampleFormat::I64 => build_input_stream::<i64>(&device, &config, channels, tx)?,
        SampleFormat::U8 => build_input_stream::<u8>(&device, &config, channels, tx)?,
        SampleFormat::U16 => build_input_stream::<u16>(&device, &config, channels, tx)?,
        SampleFormat::U32 => build_input_stream::<u32>(&device, &config, channels, tx)?,
        SampleFormat::U64 => build_input_stream::<u64>(&device, &config, channels, tx)?,
        SampleFormat::F64 => build_input_stream::<f64>(&device, &config, channels, tx)?,
        _ => return Err(format!("Unsupported input sample format: {sample_format:?}").into()),
    };

    stream.play()?;
    println!("Capturing system audio... Press Ctrl+C to stop.");

    let mut smoothed_level = 0.0f32;
    loop {
        match rx.recv_timeout(Duration::from_millis(60)) {
            Ok(level) => {
                smoothed_level = smoothed_level * 0.82 + level * 0.18;
                render_wave(smoothed_level);
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }
    }

    Ok(())
}

fn pick_capture_device(host: &cpal::Host) -> Result<cpal::Device, Box<dyn std::error::Error>> {
    struct NamedDevice {
        device: cpal::Device,
        name: String,
        normalized: String,
    }

    let mut input_devices = Vec::new();
    for device in host.input_devices()? {
        let name = device.name().unwrap_or_else(|_| String::from("<unknown>"));
        input_devices.push(NamedDevice {
            device,
            normalized: normalize_device_name(&name),
            name,
        });
    }

    if input_devices.is_empty() {
        return Err("No input capture devices available".into());
    }

    if let Some(default_monitor) = default_sink_monitor_name() {
        let target = normalize_device_name(&default_monitor);
        if let Some(index) = input_devices.iter().position(|d| {
            d.normalized.contains(&target) || target.contains(&d.normalized)
        }) {
            return Ok(input_devices.swap_remove(index).device);
        }
    }

    if default_source_is_monitor() {
        if let Some(index) = input_devices.iter().position(|d| d.normalized == "default") {
            return Ok(input_devices.swap_remove(index).device);
        }

        if let Some(index) = input_devices.iter().position(|d| d.normalized == "pipewire") {
            return Ok(input_devices.swap_remove(index).device);
        }
    }

    if let Some(index) = input_devices
        .iter()
        .position(|d| d.normalized.contains("monitor"))
    {
        return Ok(input_devices.swap_remove(index).device);
    }

    let available = input_devices
        .iter()
        .map(|d| d.name.as_str())
        .collect::<Vec<_>>()
        .join(", ");

    Err(format!(
        "No monitor capture source found. Available input devices: {available}. On PipeWire, set your default source to the sink monitor: `pactl set-default-source \"$(pactl get-default-sink).monitor\"`"
    )
    .into())
}

fn normalize_device_name(name: &str) -> String {
    name.to_lowercase().replace([' ', '-', '_', '.', ':'], "")
}

#[cfg(target_os = "linux")]
fn default_sink_monitor_name() -> Option<String> {
    let output = Command::new("pactl").args(["get-default-sink"]).output().ok()?;
    if !output.status.success() {
        return None;
    }

    let sink = String::from_utf8(output.stdout).ok()?;
    let sink = sink.trim();
    if sink.is_empty() {
        return None;
    }

    Some(format!("{sink}.monitor"))
}

#[cfg(target_os = "linux")]
fn default_source_is_monitor() -> bool {
    let output = match Command::new("pactl").args(["get-default-source"]).output() {
        Ok(output) => output,
        Err(_) => return false,
    };

    if !output.status.success() {
        return false;
    }

    let source = match String::from_utf8(output.stdout) {
        Ok(source) => source,
        Err(_) => return false,
    };

    source.trim().ends_with(".monitor")
}

#[cfg(not(target_os = "linux"))]
fn default_sink_monitor_name() -> Option<String> {
    None
}

#[cfg(not(target_os = "linux"))]
fn default_source_is_monitor() -> bool {
    false
}

fn build_input_stream<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    channels: usize,
    tx: Sender<f32>,
) -> Result<Stream, cpal::BuildStreamError>
where
    T: Sample + SizedSample,
    f32: FromSample<T>,
{
    device.build_input_stream(
        config,
        move |data: &[T], _| {
            if data.is_empty() {
                return;
            }

            let level = rms_level(data, channels);
            let _ = tx.send(level);
        },
        move |err| eprintln!("Stream error: {err}"),
        None,
    )
}

fn rms_level<T>(data: &[T], channels: usize) -> f32
where
    T: Sample,
    f32: FromSample<T>,
{
    let channel_count = channels.max(1);
    let mut sum_squares = 0.0f32;
    let mut frames = 0usize;

    for frame in data.chunks(channel_count) {
        let mut mono = 0.0f32;
        for sample in frame {
            mono += sample.to_sample::<f32>();
        }

        mono /= frame.len() as f32;
        sum_squares += mono * mono;
        frames += 1;
    }

    if frames == 0 {
        return 0.0;
    }

    (sum_squares / frames as f32).sqrt().clamp(0.0, 1.0)
}

fn render_wave(level: f32) {
    let width = 64usize;
    let filled = ((level * width as f32).round() as usize).min(width);
    let empty = width - filled;

    let bar = format!("{}{}", "#".repeat(filled), " ".repeat(empty));
    let _ = write!(io::stdout(), "\r[{bar}] {:>5.1}%", level * 100.0);
    let _ = io::stdout().flush();
}
