use audio_listener::audio_listener;
use rodio::{Decoder, OutputStreamBuilder, Sink, buffer::SamplesBuffer};
use std::fs::File;

pub mod audio_listener;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    audio_listener()
}

fn extract_samples(path: &str) -> Result<Vec<f32>, Box<dyn std::error::Error>> {
    let file = File::open(path)?;
    let decoder: Decoder<File> = Decoder::new_mp3(file)?;

    let mut samples: Vec<f32> = Vec::new();

    for sample in decoder {
        samples.push(sample);
    }

    Ok(samples)
}

fn play_samples(samples: Vec<f32>) {
    let stream_handle =
        OutputStreamBuilder::open_default_stream().expect("open default audio stream");
    let sink = Sink::connect_new(&stream_handle.mixer());

    let channels = 2;
    let sample_rate = 44100;

    let buffer = SamplesBuffer::new(channels, sample_rate, samples);
    sink.append(buffer);

    sink.sleep_until_end();
}
