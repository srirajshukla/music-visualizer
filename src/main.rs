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
        BarChart, Block, Borders, Paragraph,
    },
    Frame, Terminal,
};
use spectrum_analyzer::{
    scaling::divide_by_N, samples_fft_to_spectrum, windows::hann_window, FrequencyLimit,
    FrequencySpectrum,
};
use std::{
    collections::VecDeque,
    io,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

// --- Data Structures ---

struct BeatInfo {
    is_beat: bool,
    bpm: f32,
    total_beats: usize,
}

// --- Visualizer Trait ---

trait Visualizer {
    fn name(&self) -> &str;
    fn draw(&self, f: &mut Frame, area: Rect, spectrum: &FrequencySpectrum, beat_info: &BeatInfo);
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
            if f < 20.0 || f > 20000.0 {
                continue;
            }

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

    fn draw(&self, f: &mut Frame, area: Rect, spectrum: &FrequencySpectrum, beat_info: &BeatInfo) {
        let color = if beat_info.is_beat {
            Color::Magenta
        } else {
            Color::Cyan
        };
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
                    ctx.draw(&Line {
                        x1,
                        y1,
                        x2,
                        y2,
                        color,
                    });

                    let (x1b, y1b) = bottom_points[i];
                    let (x2b, y2b) = bottom_points[i + 1];
                    ctx.draw(&Line {
                        x1: x1b,
                        y1: y1b,
                        x2: x2b,
                        y2: y2b,
                        color,
                    });

                    if i % 2 == 0 {
                        ctx.draw(&Line {
                            x1,
                            y1,
                            x2: x1b,
                            y2: y1b,
                            color: Color::DarkGray,
                        });
                    }
                }
                if beat_info.is_beat {
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
            if f < 20.0 || f > 20000.0 {
                continue;
            }

            let log_f = f.ln();
            let bin_idx = (((log_f - min_log) / log_range) * num_bars as f32) as usize;
            let bin_idx = bin_idx.min(num_bars - 1);

            bins[bin_idx] += val;
            counts[bin_idx] += 1;
        }

        bins.iter()
            .enumerate()
            .map(|(i, &v)| {
                let avg = if counts[i] > 0 {
                    v / counts[i] as f32
                } else {
                    0.0
                };
                (avg * 1500.0) as u64
            })
            .collect()
    }
}

impl Visualizer for BarVisualizer {
    fn name(&self) -> &str {
        "Frequency Bars"
    }

    fn draw(&self, f: &mut Frame, area: Rect, spectrum: &FrequencySpectrum, beat_info: &BeatInfo) {
        let color = if beat_info.is_beat {
            Color::Yellow
        } else {
            Color::Green
        };
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
    last_beat: Instant,
    intervals: VecDeque<Duration>,
    total_beats: usize,
}

impl BeatDetector {
    fn new(history_size: usize, sensitivity: f32) -> Self {
        Self {
            energy_history: Vec::with_capacity(history_size),
            history_size,
            sensitivity,
            last_beat: Instant::now(),
            intervals: VecDeque::with_capacity(10),
            total_beats: 0,
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

        let is_beat = avg_low_energy > self.sensitivity * history_avg && avg_low_energy > 0.01;

        if is_beat {
            let now = Instant::now();
            let duration = now.duration_since(self.last_beat);
            // Ignore accidental double-triggers
            if duration.as_millis() > 300 {
                self.intervals.push_back(duration);
                if self.intervals.len() > 10 {
                    self.intervals.pop_front();
                }
                self.last_beat = now;
                self.total_beats += 1;
            }
        }

        is_beat
    }

    fn get_bpm(&self) -> f32 {
        if self.intervals.is_empty() {
            return 0.0;
        }
        let avg_ms = self.intervals.iter().map(|d| d.as_millis()).sum::<u128>() as f32
            / self.intervals.len() as f32;
        if avg_ms == 0.0 {
            0.0
        } else {
            60000.0 / avg_ms
        }
    }
}

// --- Utils ---

fn get_peak_frequency(spectrum: &FrequencySpectrum) -> (u32, f32) {
    let mut max_val = 0.0;
    let mut peak_freq = 0;
    for (freq, val) in spectrum.to_map().iter() {
        if *val > max_val {
            max_val = *val;
            peak_freq = *freq;
        }
    }
    (peak_freq, max_val)
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
    let mut show_info_panel = true;

    // 3. Main Render Loop
    loop {
        if event::poll(Duration::from_millis(16))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    match key.code {
                        KeyCode::Char('q') => break,
                        KeyCode::Char('i') => show_info_panel = !show_info_panel,
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

        let beat_info = BeatInfo {
            is_beat,
            bpm: beat_detector.get_bpm(),
            total_beats: beat_detector.total_beats,
        };

        terminal.draw(|f| {
            let layout = Layout::default()
                .direction(Direction::Vertical)
                .constraints(if show_info_panel {
                    &[Constraint::Min(0), Constraint::Length(3)][..]
                } else {
                    &[Constraint::Min(0)][..]
                })
                .split(f.area());

            if let Some(spectrum) = &spectrum_data {
                // Main Visualization
                visualizers[current_visualizer_index].draw(f, layout[0], spectrum, &beat_info);

                if show_info_panel {
                    // Info Panel
                    let (peak_freq, _peak_val) = get_peak_frequency(spectrum);
                    let info_text = format!(
                        " Peak Freq: {:>5} Hz | Est. BPM: {:>5.1} | Beats: {:>4} | Controls: [q]uit, [tab] style, [i]nfo",
                        peak_freq, beat_info.bpm, beat_info.total_beats
                    );

                    let info_panel = Paragraph::new(info_text)
                        .block(
                            Block::default()
                                .borders(Borders::ALL)
                                .title(" Audio Intelligence "),
                        )
                        .style(Style::default().fg(if is_beat {
                            Color::Magenta
                        } else {
                            Color::White
                        }));

                    f.render_widget(info_panel, layout[1]);
                }
            }
        })?;
    }

    // Cleanup
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    Ok(())
}
