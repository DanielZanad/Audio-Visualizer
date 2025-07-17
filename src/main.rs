use cpal::SampleFormat;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let host = cpal::default_host();

    // Manually match the monitor device from `pactl`
    let device = host.default_output_device().expect("no output device");

    println!("Using monitor device: {}", device.name()?);

    let config = device.default_output_config()?.config();
    let sample_format = device.default_output_config()?.sample_format();

    let err_fn = |err| eprintln!("Stream error: {}", err);

    let stream = match sample_format {
        SampleFormat::F32 => device.build_input_stream(
            &config,
            move |data: &[f32], _| {
                println!("Received {:?} samples", data);
            },
            err_fn,
            None,
        )?,
        SampleFormat::I16 => device.build_input_stream(
            &config,
            move |data: &[i16], _| {
                println!("Received {} samples", data.len());
            },
            err_fn,
            None,
        )?,
        SampleFormat::U16 => device.build_input_stream(
            &config,
            move |data: &[u16], _| {
                println!("Received {} samples", data.len());
            },
            err_fn,
            None,
        )?,
        SampleFormat::I8 => todo!(),
        SampleFormat::I24 => todo!(),
        SampleFormat::I32 => todo!(),
        SampleFormat::I64 => todo!(),
        SampleFormat::U8 => todo!(),
        SampleFormat::U32 => todo!(),
        SampleFormat::U64 => todo!(),
        SampleFormat::F64 => todo!(),
        _ => todo!(),
    };

    stream.play()?;

    println!("Capturing system audio... Press Ctrl+C to stop.");
    std::thread::park(); // Keeps the main thread alive

    Ok(())
}
