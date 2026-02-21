use anyhow::Result;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use crossterm::{
    event::{self, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    widgets::{
        canvas::{Canvas, Line},
        Block, Borders,
    },
    Terminal,
};
use spectrum_analyzer::{
    scaling::divide_by_N, samples_fft_to_spectrum, windows::hann_window, FrequencyLimit,
};
use std::{
    io,
    sync::{Arc, Mutex},
    time::Duration,
};

struct BeatDetector {
    energy_history: Vec<f32>,
    history_size: usize,
    sensitivity: f32,
}

impl BeatDetector {
    fn new(history_size: usize, sensitivity: f32) -> Self {
        Self {
            energy_history: Vec::with_capacity(history_size),
            history_size,
            sensitivity,
        }
    }

    fn detect(&mut self, spectrum_data: &spectrum_analyzer::FrequencySpectrum) -> bool {
        // Focus on low frequencies for beat detection (e.g., 20Hz - 150Hz)
        let mut low_energy = 0.0;
        let mut count = 0;
        for (freq, val) in spectrum_data.to_map().iter() {
            let f = *freq as f32;
            let v = *val;
            if f >= 20.0 && f <= 150.0 {
                low_energy += v;
                count += 1;
            }
        }

        if count == 0 {
            return false;
        }

        let avg_low_energy = low_energy / count as f32;

        if self.energy_history.is_empty() {
            self.energy_history.push(avg_low_energy);
            return false;
        }

        let history_avg: f32 =
            self.energy_history.iter().sum::<f32>() / self.energy_history.len() as f32;

        self.energy_history.push(avg_low_energy);
        if self.energy_history.len() > self.history_size {
            self.energy_history.remove(0);
        }

        // Detect beat if current low energy is significantly higher than average
        avg_low_energy > self.sensitivity * history_avg && avg_low_energy > 0.01
    }
}

fn main() -> Result<()> {
    // 1. Setup Audio Capture
    let host = cpal::default_host();
    let device = host
        .default_output_device()
        .expect("No output device found");

    println!("Capturing audio from: {}", device.description()?);

    let config: cpal::StreamConfig = device.default_output_config()?.into();

    // Shared buffer to send samples from Audio Thread -> UI Thread
    let samples = Arc::new(Mutex::new(Vec::new()));
    let samples_clone = samples.clone();

    let stream = device.build_input_stream(
        &config,
        move |data: &[f32], _: &_| {
            let mut s = samples_clone.lock().unwrap();
            s.extend_from_slice(data);
            if s.len() > 4096 {
                let keep = s.len() - 4096;
                s.drain(0..keep);
            }
        },
        |err| eprintln!("Stream error: {}", err),
        None,
    )?;

    stream.play()?;

    // 2. Setup Terminal UI
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut beat_detector = BeatDetector::new(43, 1.5); // ~1 second history at 60fps
    let mut is_beat = false;
    let mut beat_timer = 0;

    // 3. Main Render Loop
    loop {
        // Handle Input
        if event::poll(Duration::from_millis(16))? {
            if let Event::Key(key) = event::read()? {
                if key.code == KeyCode::Char('q') {
                    break;
                }
            }
        }

        // Process Audio Data
        let spectrum_data = {
            let s = samples.lock().unwrap();
            if s.len() >= 2048 {
                let window = &s[s.len() - 2048..];
                let hann_window = hann_window(window);

                samples_fft_to_spectrum(
                    &hann_window,
                    config.sample_rate,
                    FrequencyLimit::Range(20., 20_000.),
                    Some(&divide_by_N),
                )
                .ok()
            } else {
                None
            }
        };

        if let Some(ref spectrum) = spectrum_data {
            if beat_detector.detect(spectrum) {
                is_beat = true;
                beat_timer = 5; // Beat effect lasts for 5 frames
            }
        }

        if beat_timer > 0 {
            beat_timer -= 1;
        } else {
            is_beat = false;
        }

        // Draw UI
        terminal.draw(|f| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Percentage(100)].as_ref())
                .split(f.area());

            let color = if is_beat { Color::Magenta } else { Color::Cyan };

            if let Some(spectrum) = spectrum_data {
                let raw_data = spectrum.to_map();
                let mut top_points: Vec<(f64, f64)> = Vec::new();
                let mut bottom_points: Vec<(f64, f64)> = Vec::new();

                let mid_y = 25.0;
                let step = raw_data.len() / 60;
                for (i, (_freq, val)) in raw_data.iter().step_by(step.max(1)).enumerate() {
                    let x = i as f64;
                    let height = (*val * 150.0) as f64; // Increased scale
                    top_points.push((x, mid_y + height));
                    bottom_points.push((x, mid_y - height));
                }

                let canvas = Canvas::default()
                    .block(
                        Block::default()
                            .title("Dynamic Music Visualization")
                            .borders(Borders::ALL)
                            .border_style(Style::default().fg(color)),
                    )
                    .x_bounds([0.0, top_points.len() as f64])
                    .y_bounds([0.0, 50.0])
                    .paint(|ctx| {
                        for i in 0..top_points.len().saturating_sub(1) {
                            // Top curve
                            let (x1, y1) = top_points[i];
                            let (x2, y2) = top_points[i + 1];
                            ctx.draw(&Line { x1, y1, x2, y2, color });

                            // Bottom curve (mirror)
                            let (x1b, y1b) = bottom_points[i];
                            let (x2b, y2b) = bottom_points[i + 1];
                            ctx.draw(&Line { x1: x1b, y1: y1b, x2: x2b, y2: y2b, color });
                            
                            // Connecting lines for a "ribbon" effect
                            if i % 2 == 0 {
                                ctx.draw(&Line { x1, y1, x2: x1b, y2: y1b, color: Color::DarkGray });
                            }
                        }
                        
                        if is_beat {
                             ctx.print(0.0, 45.0, ">>> BEAT <<<");
                        }
                    });

                f.render_widget(canvas, chunks[0]);
            }
        })?;
    }

    // Cleanup
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    Ok(())
}
