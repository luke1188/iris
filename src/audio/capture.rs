//! Cross-platform audio capture via cpal.
//!
//! On Windows, WASAPI input devices (including virtual cables like Serato VAC)
//! are enumerated. True speaker loopback is best-effort: devices whose names
//! suggest loopback / "what u hear" / stereo mix are flagged. Dedicated
//! WASAPI loopback capture can be expanded in a later phase.

use anyhow::{anyhow, Context, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Device, SampleFormat, Stream, StreamConfig};
use parking_lot::Mutex;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

const RING_CAPACITY: usize = 48_000 * 2; // ~2s at 48kHz mono

#[derive(Debug, Clone)]
pub struct AudioDeviceInfo {
    /// Stable-ish id used for selection (device name for cpal).
    pub id: String,
    pub name: String,
    pub is_input: bool,
    pub is_loopback: bool,
    pub is_default: bool,
}

struct SharedBuffer {
    samples: VecDeque<f32>,
    sample_rate: u32,
    channels: u16,
}

pub struct AudioCapture {
    host_name: String,
    stream: Option<Stream>,
    buffer: Arc<Mutex<SharedBuffer>>,
    running: Arc<AtomicBool>,
    active_device: Option<String>,
}

impl AudioCapture {
    pub fn new() -> Self {
        let host = cpal::default_host();
        Self {
            host_name: format!("{:?}", host.id()),
            stream: None,
            buffer: Arc::new(Mutex::new(SharedBuffer {
                samples: VecDeque::with_capacity(RING_CAPACITY),
                sample_rate: 48_000,
                channels: 1,
            })),
            running: Arc::new(AtomicBool::new(false)),
            active_device: None,
        }
    }

    pub fn host_name(&self) -> &str {
        &self.host_name
    }

    pub fn list_devices(&self) -> Vec<AudioDeviceInfo> {
        let host = cpal::default_host();
        let default_in = host.default_input_device().and_then(|d| d.name().ok());

        let mut devices = Vec::new();

        if let Ok(inputs) = host.input_devices() {
            for device in inputs {
                let name = match device.name() {
                    Ok(n) => n,
                    Err(_) => continue,
                };
                let is_loopback = looks_like_loopback(&name);
                let is_default = default_in.as_ref() == Some(&name);
                devices.push(AudioDeviceInfo {
                    id: name.clone(),
                    name,
                    is_input: true,
                    is_loopback,
                    is_default,
                });
            }
        }

        // Sort: default first, then alphabetical
        devices.sort_by(|a, b| {
            b.is_default
                .cmp(&a.is_default)
                .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        });

        devices
    }

    pub fn sample_rate(&self) -> Option<u32> {
        if self.stream.is_some() {
            Some(self.buffer.lock().sample_rate)
        } else {
            None
        }
    }

    pub fn channels(&self) -> Option<u16> {
        if self.stream.is_some() {
            Some(self.buffer.lock().channels)
        } else {
            None
        }
    }

    pub fn active_device(&self) -> Option<&str> {
        self.active_device.as_deref()
    }

    pub fn stop(&mut self) {
        // Signal callbacks to bail ASAP, then drop the stream so the device
        // is released before the next process launch (critical on WASAPI/CoreAudio).
        self.running.store(false, Ordering::SeqCst);
        if let Some(stream) = self.stream.take() {
            // Explicit drop; avoid holding other locks while joining the audio thread.
            drop(stream);
        }
        self.active_device = None;
        if let Some(mut buf) = self.buffer.try_lock() {
            buf.samples.clear();
        }
    }

    pub fn start(&mut self, device_id: &str) -> Result<()> {
        self.stop();

        let host = cpal::default_host();
        let device = find_input_device(&host, device_id)
            .with_context(|| format!("Audio device not found: {device_id}"))?;

        let config = preferred_input_config(&device)?;
        let sample_format = config.sample_format();
        let stream_config: StreamConfig = config.into();
        let channels = stream_config.channels;
        let sample_rate = stream_config.sample_rate.0;

        {
            let mut buf = self.buffer.lock();
            buf.sample_rate = sample_rate;
            buf.channels = channels;
            buf.samples.clear();
        }

        self.running.store(true, Ordering::SeqCst);
        let buffer = Arc::clone(&self.buffer);
        let running = Arc::clone(&self.running);

        let err_fn = |e| log::error!("Audio stream error: {e}");

        let stream = match sample_format {
            SampleFormat::F32 => build_stream::<f32>(&device, &stream_config, buffer, running, err_fn)?,
            SampleFormat::I16 => build_stream::<i16>(&device, &stream_config, buffer, running, err_fn)?,
            SampleFormat::U16 => build_stream::<u16>(&device, &stream_config, buffer, running, err_fn)?,
            SampleFormat::I32 => build_stream::<i32>(&device, &stream_config, buffer, running, err_fn)?,
            SampleFormat::U32 => build_stream::<u32>(&device, &stream_config, buffer, running, err_fn)?,
            other => return Err(anyhow!("Unsupported sample format: {other:?}")),
        };

        stream.play().context("Failed to play audio stream")?;
        self.stream = Some(stream);
        self.active_device = Some(device_id.to_string());
        log::info!(
            "Capturing from '{device_id}' ({sample_rate} Hz, {channels} ch, host {})",
            self.host_name
        );
        Ok(())
    }

    /// Drain available mono samples from the ring buffer (downmixed).
    /// Returns `None` if the stream is not running or buffer is empty.
    pub fn drain_samples(&self) -> Option<Vec<f32>> {
        if !self.running.load(Ordering::SeqCst) {
            return None;
        }
        let mut buf = self.buffer.lock();
        if buf.samples.is_empty() {
            return None;
        }
        let mut out = Vec::with_capacity(buf.samples.len());
        out.extend(buf.samples.drain(..));
        Some(out)
    }
}

fn find_input_device(host: &cpal::Host, device_id: &str) -> Option<Device> {
    host.input_devices().ok()?.find(|d| {
        d.name()
            .map(|n| n == device_id)
            .unwrap_or(false)
    })
}

fn preferred_input_config(device: &Device) -> Result<cpal::SupportedStreamConfig> {
    // Prefer default input config; fall back to first supported.
    if let Ok(cfg) = device.default_input_config() {
        return Ok(cfg);
    }
    let mut configs = device
        .supported_input_configs()
        .context("No supported input configs")?;
    let range = configs
        .next()
        .ok_or_else(|| anyhow!("Device reports no input configs"))?;
    Ok(range.with_max_sample_rate())
}

fn build_stream<T>(
    device: &Device,
    config: &StreamConfig,
    buffer: Arc<Mutex<SharedBuffer>>,
    running: Arc<AtomicBool>,
    err_fn: impl FnMut(cpal::StreamError) + Send + 'static,
) -> Result<Stream>
where
    T: cpal::Sample + cpal::SizedSample + Send + 'static,
    f32: cpal::FromSample<T>,
{
    let channels = config.channels as usize;
    let stream = device.build_input_stream(
        config,
        move |data: &[T], _| {
            if !running.load(Ordering::Relaxed) {
                return;
            }
            let mut buf = buffer.lock();
            // Downmix to mono and push into ring buffer
            let frames = data.len() / channels.max(1);
            for frame in 0..frames {
                let mut sum = 0.0_f32;
                for ch in 0..channels {
                    let s: f32 = cpal::Sample::from_sample(data[frame * channels + ch]);
                    sum += s;
                }
                let mono = sum / channels.max(1) as f32;
                if buf.samples.len() >= RING_CAPACITY {
                    buf.samples.pop_front();
                }
                buf.samples.push_back(mono);
            }
        },
        err_fn,
        None,
    )?;
    Ok(stream)
}

fn looks_like_loopback(name: &str) -> bool {
    let n = name.to_lowercase();
    n.contains("loopback")
        || n.contains("stereo mix")
        || n.contains("what u hear")
        || n.contains("wave out")
        || n.contains("cable output")
        || n.contains("vb-audio")
        || n.contains("voicemeeter")
        || n.contains("serato")
}
