use anyhow::Result;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    widgets::{Block, Borders, Paragraph},
    Terminal,
};
use spectrum_analyzer::{
    scaling::divide_by_N, samples_fft_to_spectrum, windows::hann_window, FrequencyLimit,
    FrequencySpectrum,
};
use std::{
    collections::VecDeque,
    io,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};

mod visualizers;
use visualizers::{
    bars::BarVisualizer,
    liquid::LiquidWorld,
    particles::{HorizontalParticles, MixedParticles, VerticalParticles},
    radial::RadialVisualizer,
    waveform::WaveformVisualizer,
    waves::{LissajousEnhanced, LissajousInterference, ResonantHelix, SpectralRibbons},
    BeatInfo, Visualizer,
};

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
        
        // Use a weighted average where lower frequencies (20-60Hz) are prioritized
        // as they represent the "thump" of the kick drum more accurately.
        for (freq, val) in spectrum_data.to_map().iter() {
            let f = *freq as f32;
            let v = *val;
            if f >= 20.0 && f <= 150.0 {
                let weight = if f <= 60.0 { 1.5 } else { 1.0 };
                low_energy += v * weight;
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

        // Calculate both average and variance for a more dynamic threshold
        let history_avg: f32 =
            self.energy_history.iter().sum::<f32>() / self.energy_history.len() as f32;
        
        let variance: f32 = self.energy_history.iter()
            .map(|e| (e - history_avg).powi(2))
            .sum::<f32>() / self.energy_history.len() as f32;
        
        // A "beat" is a peak that stands out significantly from the local noise floor.
        // We use a combination of sensitivity * average and a variance-based offset.
        let dynamic_threshold = self.sensitivity * history_avg + variance.sqrt() * 0.5;

        self.energy_history.push(avg_low_energy);
        if self.energy_history.len() > self.history_size {
            self.energy_history.remove(0);
        }

        let is_beat = avg_low_energy > dynamic_threshold && avg_low_energy > 0.01;

        if is_beat {
            let now = Instant::now();
            let duration = now.duration_since(self.last_beat);
            // Limit to ~200 BPM (300ms) to avoid double triggers
            if duration.as_millis() > 300 {
                self.intervals.push_back(duration);
                if self.intervals.len() > 15 {
                    self.intervals.pop_front();
                }
                self.last_beat = now;
                self.total_beats += 1;
            }
        }

        is_beat
    }

    fn get_bpm(&self) -> f32 {
        if self.intervals.len() < 3 {
            return 0.0;
        }
        
        // Use a median-like approach: sort intervals and pick the middle range 
        // to ignore outliers (missed beats or accidental double triggers).
        let mut sorted_intervals: Vec<u128> = self.intervals.iter()
            .map(|d| d.as_millis())
            .collect();
        sorted_intervals.sort_unstable();
        
        let mid = sorted_intervals.len() / 2;
        let median_ms = if sorted_intervals.len() % 2 == 0 {
            (sorted_intervals[mid - 1] + sorted_intervals[mid]) as f32 / 2.0
        } else {
            sorted_intervals[mid] as f32
        };

        if median_ms == 0.0 {
            0.0
        } else {
            60000.0 / median_ms
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

fn setup_audio_stream(
    samples: Arc<Mutex<Vec<f32>>>,
    restart_flag: Arc<AtomicBool>,
) -> Result<(cpal::Stream, cpal::StreamConfig)> {
    let host = cpal::default_host();
    let device = host
        .default_output_device()
        .ok_or_else(|| anyhow::anyhow!("No output device found"))?;

    let config: cpal::StreamConfig = device.default_output_config()?.into();
    let channels = config.channels as usize;

    let samples_clone = samples.clone();
    let restart_flag_clone = restart_flag.clone();

    let stream = device.build_input_stream(
        &config,
        move |data: &[f32], _: &_| {
            let mut s = samples_clone.lock().unwrap();
            if channels > 1 {
                for frame in data.chunks_exact(channels) {
                    let mono: f32 = frame.iter().sum::<f32>() / channels as f32;
                    s.push(mono);
                }
            } else {
                s.extend_from_slice(data);
            }
            
            if s.len() > 4096 {
                let keep = s.len() - 4096;
                s.drain(0..keep);
            }
        },
        move |_err| {
            restart_flag_clone.store(true, Ordering::SeqCst);
        },
        None,
    )?;

    stream.play()?;
    Ok((stream, config))
}

fn main() -> Result<()> {
    // 1. Setup Audio Capture
    let samples = Arc::new(Mutex::new(Vec::new()));
    let restart_flag = Arc::new(AtomicBool::new(false));

    let (mut _stream, mut config) = match setup_audio_stream(samples.clone(), restart_flag.clone()) {
        Ok(res) => (Some(res.0), res.1),
        Err(_) => (
            None,
            cpal::StreamConfig {
                channels: 1,
                sample_rate: 44100,
                buffer_size: cpal::BufferSize::Default,
            },
        ),
    };

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
    let visualizers: Vec<Box<dyn Visualizer>> = vec![
        Box::new(WaveformVisualizer),
        Box::new(BarVisualizer::new()),
        Box::new(RadialVisualizer::new()),
        Box::new(VerticalParticles::new()),
        Box::new(HorizontalParticles::new()),
        Box::new(MixedParticles::new()),
        Box::new(LiquidWorld::new()),
        Box::new(SpectralRibbons::new()),
        Box::new(ResonantHelix::new()),
        Box::new(LissajousInterference::new()),
        Box::new(LissajousEnhanced::new()),
    ];
    let mut current_visualizer_index = 0;
    let mut show_info_panel = true;

    let mut last_info_update = Instant::now();
    let mut displayed_peak_freq = 0;

    // 3. Main Render Loop
    loop {
        if _stream.is_none() || restart_flag.load(Ordering::SeqCst) {
            if let Ok((new_stream, new_config)) =
                setup_audio_stream(samples.clone(), restart_flag.clone())
            {
                _stream = Some(new_stream);
                config = new_config;
                restart_flag.store(false, Ordering::SeqCst);
                // Clear old samples
                let mut s = samples.lock().unwrap();
                s.clear();
            }
        }

        if event::poll(Duration::from_millis(16))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    match key.code {
                        KeyCode::Char('q') => break,
                        KeyCode::Char('i') => show_info_panel = !show_info_panel,
                        KeyCode::Tab | KeyCode::Right => {
                            current_visualizer_index =
                                (current_visualizer_index + 1) % visualizers.len();
                        }
                        KeyCode::BackTab | KeyCode::Left => {
                            current_visualizer_index =
                                (current_visualizer_index + visualizers.len() - 1) % visualizers.len();
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
                    // Update peak frequency only every 200ms to keep it readable
                    if last_info_update.elapsed() >= Duration::from_millis(200) {
                        let (peak_freq, _peak_val) = get_peak_frequency(spectrum);
                        displayed_peak_freq = peak_freq;
                        last_info_update = Instant::now();
                    }

                    let info_text = format!(
                        " Peak Freq: {:>5} Hz | Est. BPM: {:>5.1} | Beats: {:>4} | Controls: [q]uit, [right/tab] next, [left/shift-tab] prev, [i]nfo",
                        displayed_peak_freq, beat_info.bpm, beat_info.total_beats
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
