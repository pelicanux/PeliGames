//! The notification chime, played natively. The webview's WebAudio depends on
//! GStreamer elements that a system may lack, and a missing one used to abort
//! the whole web process; the same two notes now come from Rust and cpal.
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::{
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};

static PLAYING: AtomicBool = AtomicBool::new(false);

/// Two short square-wave notes (E5 then B5), as the earlier WebAudio chime:
/// a 6 ms attack to `0.08 × volume` and an exponential fade over 90 ms.
fn chime(rate: u32, volume: u32) -> Vec<f32> {
    let rate = rate as f32;
    let peak = 0.08 * volume.min(100) as f32 / 100.;
    let length = (rate * 0.25) as usize;
    let mut samples = vec![0f32; length];
    for (offset, frequency) in [(0.0f32, 659.25f32), (0.115, 987.77)] {
        let start = (offset * rate) as usize;
        for index in 0..(rate * 0.1) as usize {
            let Some(sample) = samples.get_mut(start + index) else { break };
            let time = index as f32 / rate;
            let envelope = if time < 0.006 {
                peak * time / 0.006
            } else {
                // From the peak at 6 ms down to 0.0001 at 90 ms, then silent.
                let fade = ((time - 0.006) / 0.084).min(1.);
                peak * (0.0001f32 / peak.max(0.0001)).powf(fade)
            };
            let square = if (time * frequency).fract() < 0.5 { 1. } else { -1. };
            *sample += square * envelope;
        }
    }
    samples
}

fn play(volume: u32) -> Result<(), String> {
    let device = cpal::default_host().default_output_device().ok_or("Sem saída de áudio.")?;
    let supported = device.default_output_config().map_err(|_| "Saída de áudio indisponível.")?;
    let config: cpal::StreamConfig = supported.config();
    let samples = chime(config.sample_rate.0, volume);
    let seconds = samples.len() as f32 / config.sample_rate.0 as f32;
    let channels = config.channels as usize;
    let mut position = 0usize;
    let mut next = move || {
        let value = samples.get(position).copied().unwrap_or(0.);
        position += 1;
        value
    };
    let error = |_| {};
    let stream = match supported.sample_format() {
        cpal::SampleFormat::F32 => device.build_output_stream(&config, move |data: &mut [f32], _| {
            for frame in data.chunks_mut(channels) { let value = next(); frame.fill(value); }
        }, error, None),
        cpal::SampleFormat::I16 => device.build_output_stream(&config, move |data: &mut [i16], _| {
            for frame in data.chunks_mut(channels) { let value = (next() * i16::MAX as f32) as i16; frame.fill(value); }
        }, error, None),
        cpal::SampleFormat::U16 => device.build_output_stream(&config, move |data: &mut [u16], _| {
            for frame in data.chunks_mut(channels) { let value = ((next() + 1.) * 0.5 * u16::MAX as f32) as u16; frame.fill(value); }
        }, error, None),
        _ => return Err("Formato de áudio sem suporte.".into()),
    }
    .map_err(|_| "Não foi possível abrir a saída de áudio.")?;
    stream.play().map_err(|_| "Não foi possível tocar o som.")?;
    // A little longer than the chime, so the device drains its buffer.
    std::thread::sleep(Duration::from_secs_f32(seconds + 0.15));
    Ok(())
}

/// Plays the chime once; a chime already playing makes this one a no-op.
#[tauri::command]
pub fn play_notification_sound(volume: u32) -> Result<bool, String> {
    if volume == 0 || volume > 100 {
        return Ok(false);
    }
    if PLAYING.swap(true, Ordering::AcqRel) {
        return Ok(false);
    }
    std::thread::spawn(move || {
        let _ = play(volume);
        PLAYING.store(false, Ordering::Release);
    });
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::chime;

    #[test]
    fn chime_is_short_quiet_and_scales_with_volume() {
        let loud = chime(48000, 100);
        assert_eq!(loud.len(), 12000);
        let peak = loud.iter().fold(0f32, |max, value| max.max(value.abs()));
        assert!(peak > 0.07 && peak <= 0.081, "{peak}");
        let quiet = chime(48000, 35);
        let quiet_peak = quiet.iter().fold(0f32, |max, value| max.max(value.abs()));
        assert!((quiet_peak / peak - 0.35).abs() < 0.01);
        // Silent after both notes.
        assert!(loud[(0.215 * 48000.) as usize + 10..].iter().all(|value| *value == 0.));
    }
}
