use anyhow::Result;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    widgets::{
        canvas::{Canvas, Line},
        BarChart, Block, Borders,
    },
    Frame, Terminal,
};
use spectrum_analyzer::{
    scaling::divide_by_N, samples_fft_to_spectrum, windows::hann_window, FrequencyLimit,
    FrequencySpectrum,
};
use std::{
    io,
    sync::{Arc, Mutex},
    time::Duration,
};

// --- Visualizer Trait ---

trait Visualizer {
    fn name(&self) -> &str;
    fn draw(&self, f: &mut Frame, area: Rect, spectrum: &FrequencySpectrum, is_beat: bool);
}

// --- Visualizer Implementations ---

struct WaveformVisualizer;

impl WaveformVisualizer {
    fn get_log_points(&self, spectrum: &FrequencySpectrum, num_bins: usize) -> Vec<f32> {
        let mut bins = vec![0.0f32; num_bins];
        let mut counts = vec![0; num_bins];
        
        let min_log = 20.0f32.ln();
        let max_log = 20000.0f32.ln();
        let log_range = max_log - min_log;

        for (freq, val) in spectrum.to_map().iter() {
            let f = *freq as f32;
            if f < 20.0 || f > 20000.0 { continue; }
            
            // Map frequency to a logarithmic bin index
            let log_f = f.ln();
            let bin_idx = (((log_f - min_log) / log_range) * num_bins as f32) as usize;
            let bin_idx = bin_idx.min(num_bins - 1);
            
            bins[bin_idx] += val;
            counts[bin_idx] += 1;
        }

        // Average and scale
        for i in 0..num_bins {
            if counts[i] > 0 {
                bins[i] /= counts[i] as f32;
            }
        }
        bins
    }
}

impl Visualizer for WaveformVisualizer {
    fn name(&self) -> &str {
        "Mirrored Waveform"
    }

    fn draw(&self, f: &mut Frame, area: Rect, spectrum: &FrequencySpectrum, is_beat: bool) {
        let color = if is_beat { Color::Magenta } else { Color::Cyan };
        let bins = self.get_log_points(spectrum, 60);
        
        let mut top_points: Vec<(f64, f64)> = Vec::new();
        let mut bottom_points: Vec<(f64, f64)> = Vec::new();

        let mid_y = 25.0;
        for (i, val) in bins.iter().enumerate() {
            let x = i as f64;
            let height = (*val * 200.0) as f64; // Adjusted scale for log bins
            top_points.push((x, mid_y + height));
            bottom_points.push((x, mid_y - height));
        }

        let canvas = Canvas::default()
            .block(
                Block::default()
                    .title(format!(" Style: {} ", self.name()))
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(color)),
            )
            .x_bounds([0.0, bins.len() as f64])
            .y_bounds([0.0, 50.0])
            .paint(|ctx| {
                for i in 0..top_points.len().saturating_sub(1) {
                    let (x1, y1) = top_points[i];
                    let (x2, y2) = top_points[i + 1];
                    ctx.draw(&Line { x1, y1, x2, y2, color });

                    let (x1b, y1b) = bottom_points[i];
                    let (x2b, y2b) = bottom_points[i + 1];
                    ctx.draw(&Line { x1: x1b, y1: y1b, x2: x2b, y2: y2b, color });

                    if i % 2 == 0 {
                        ctx.draw(&Line { x1, y1, x2: x1b, y2: y1b, color: Color::DarkGray });
                    }
                }
                if is_beat {
                    ctx.print(0.0, 45.0, ">>> BEAT <<<");
                }
            });

        f.render_widget(canvas, area);
    }
}

struct BarVisualizer;

impl BarVisualizer {
    fn get_log_bars(&self, spectrum: &FrequencySpectrum, num_bars: usize) -> Vec<u64> {
        let mut bins = vec![0.0f32; num_bars];
        let mut counts = vec![0; num_bars];
        
        let min_log = 20.0f32.ln();
        let max_log = 20000.0f32.ln();
        let log_range = max_log - min_log;

        for (freq, val) in spectrum.to_map().iter() {
            let f = *freq as f32;
            if f < 20.0 || f > 20000.0 { continue; }
            
            let log_f = f.ln();
            let bin_idx = (((log_f - min_log) / log_range) * num_bars as f32) as usize;
            let bin_idx = bin_idx.min(num_bars - 1);
            
            bins[bin_idx] += val;
            counts[bin_idx] += 1;
        }

        bins.iter().enumerate().map(|(i, &v)| {
            let avg = if counts[i] > 0 { v / counts[i] as f32 } else { 0.0 };
            (avg * 1500.0) as u64
        }).collect()
    }
}

impl Visualizer for BarVisualizer {
    fn name(&self) -> &str {
        "Frequency Bars"
    }

    fn draw(&self, f: &mut Frame, area: Rect, spectrum: &FrequencySpectrum, is_beat: bool) {
        let color = if is_beat { Color::Yellow } else { Color::Green };
        let bar_heights = self.get_log_bars(spectrum, 24);
        
        let bars: Vec<(&str, u64)> = bar_heights.iter().map(|&h| ("", h)).collect();

        let barchart = BarChart::default()
            .block(
                Block::default()
                    .title(format!(" Style: {} ", self.name()))
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(color)),
            )
            .data(&bars)
            .bar_width(area.width / 24)
            .bar_style(Style::default().fg(color))
            .value_style(Style::default().fg(Color::Black).bg(color));

        f.render_widget(barchart, area);
    }
}

// --- Beat Detector ---

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

    fn detect(&mut self, spectrum_data: &FrequencySpectrum) -> bool {
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

    let mut beat_detector = BeatDetector::new(43, 1.5);
    let mut is_beat = false;
    let mut beat_timer = 0;

    // Visualizers setup
    let visualizers: Vec<Box<dyn Visualizer>> =
        vec![Box::new(WaveformVisualizer), Box::new(BarVisualizer)];
    let mut current_visualizer_index = 0;

    // 3. Main Render Loop
    loop {
        if event::poll(Duration::from_millis(16))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    match key.code {
                        KeyCode::Char('q') => break,
                        KeyCode::Tab | KeyCode::Char('n') => {
                            current_visualizer_index =
                                (current_visualizer_index + 1) % visualizers.len();
                        }
                        _ => {}
                    }
                }
            }
        }

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
                beat_timer = 5;
            }
        }

        if beat_timer > 0 {
            beat_timer -= 1;
        } else {
            is_beat = false;
        }

        terminal.draw(|f| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(0)].as_ref())
                .split(f.area());

            if let Some(spectrum) = &spectrum_data {
                visualizers[current_visualizer_index].draw(f, chunks[0], spectrum, is_beat);
            }
        })?;
    }

    // Cleanup
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    Ok(())
}
