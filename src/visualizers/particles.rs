use super::{BeatInfo, Visualizer};
use ratatui::{
    layout::Rect,
    style::Color,
    widgets::canvas::{Canvas, Points},
    Frame,
};
use spectrum_analyzer::FrequencySpectrum;
use std::sync::Mutex;
use rand::random_range;

struct Particle {
    x: f64,
    y: f64,
    vx: f64,
    vy: f64,
    life: f32,
    color: Color,
}

pub struct VerticalParticles {
    particles: Mutex<Vec<Particle>>,
}

impl VerticalParticles {
    pub fn new() -> Self {
        Self {
            particles: Mutex::new(Vec::with_capacity(300)),
        }
    }
}

pub struct HorizontalParticles {
    particles: Mutex<Vec<Particle>>,
}

impl HorizontalParticles {
    pub fn new() -> Self {
        Self {
            particles: Mutex::new(Vec::with_capacity(300)),
        }
    }
}

pub struct MixedParticles {
    particles: Mutex<Vec<Particle>>,
}

impl MixedParticles {
    pub fn new() -> Self {
        Self {
            particles: Mutex::new(Vec::with_capacity(300)),
        }
    }
}

// --- Common Helper ---

fn get_log_points(spectrum: &FrequencySpectrum, num_bins: usize) -> Vec<f32> {
    let mut bins = vec![0.0f32; num_bins];
    let mut counts = vec![0; num_bins];
    let min_log = 20.0f32.ln();
    let max_log = 20000.0f32.ln();
    let log_range = max_log - min_log;

    for (freq, val) in spectrum.to_map().iter() {
        let f = *freq as f32;
        if f < 20.0 || f > 20000.0 { continue; }
        let log_f = f.ln();
        let bin_idx = (((log_f - min_log) / log_range) * num_bins as f32) as usize;
        let bin_idx = bin_idx.min(num_bins - 1);
        bins[bin_idx] += val;
        counts[bin_idx] += 1;
    }

    for i in 0..num_bins {
        if counts[i] > 0 { bins[i] /= counts[i] as f32; }
    }
    bins
}

fn get_color_for_freq(x: usize, num_bins: usize) -> Color {
    let hue = x as f32 / num_bins as f32;
    if hue < 0.2 { Color::Red }
    else if hue < 0.4 { Color::Yellow }
    else if hue < 0.7 { Color::Green }
    else if hue < 0.9 { Color::Cyan }
    else { Color::White }
}

// --- Implementation: Vertical ---

impl Visualizer for VerticalParticles {
    fn name(&self) -> &str {
        "Particles: Rain"
    }

    fn draw(&self, f: &mut Frame, area: Rect, spectrum: &FrequencySpectrum, beat_info: &BeatInfo) {
        let num_bins = 80;
        let bins = get_log_points(spectrum, num_bins);
        let mut particles = self.particles.lock().unwrap();

        for p in particles.iter_mut() {
            p.y += p.vy;
            p.life -= 0.02;
            if beat_info.is_beat { p.y += p.vy * 1.5; }
        }
        particles.retain(|p| p.life > 0.0 && p.y >= 0.0 && p.y <= 50.0);

        for (x, &val) in bins.iter().enumerate() {
            let freq_boost = 1.0 + (x as f32 / num_bins as f32) * 4.0;
            let adjusted_val = val * freq_boost;
            if adjusted_val > 0.01 {
                if random_range(0.0..1.0) < (adjusted_val * 10.0) as f64 {
                    particles.push(Particle {
                        x: x as f64,
                        y: 25.0,
                        vx: 0.0,
                        vy: random_range(-1.0..1.0),
                        life: 1.0,
                        color: get_color_for_freq(x, num_bins),
                    });
                }
            }
        }

        let canvas = Canvas::default()
            .block(ratatui::widgets::Block::default().title(format!(" Style: {} ", self.name())).borders(ratatui::widgets::Borders::ALL))
            .x_bounds([0.0, num_bins as f64]).y_bounds([0.0, 50.0])
            .paint(|ctx| {
                for p in particles.iter() {
                    ctx.draw(&Points { coords: &[(p.x, p.y)], color: p.color });
                }
            });
        f.render_widget(canvas, area);
    }
}

// --- Implementation: Horizontal ---

impl Visualizer for HorizontalParticles {
    fn name(&self) -> &str {
        "Particles: Flow"
    }

    fn draw(&self, f: &mut Frame, area: Rect, spectrum: &FrequencySpectrum, beat_info: &BeatInfo) {
        let num_bins = 80;
        let bins = get_log_points(spectrum, num_bins);
        let mut particles = self.particles.lock().unwrap();

        for p in particles.iter_mut() {
            p.x += p.vx;
            p.life -= 0.02;
            if beat_info.is_beat { p.x += p.vx * 2.0; }
        }
        particles.retain(|p| p.life > 0.0 && p.x >= 0.0 && p.x <= num_bins as f64);

        for (x, &val) in bins.iter().enumerate() {
            let freq_boost = 1.0 + (x as f32 / num_bins as f32) * 4.0;
            let adjusted_val = val * freq_boost;
            if adjusted_val > 0.01 {
                if random_range(0.0..1.0) < (adjusted_val * 15.0) as f64 {
                    particles.push(Particle {
                        x: 0.0,
                        y: (x as f64 / num_bins as f64) * 50.0,
                        vx: random_range(0.5..1.5),
                        vy: 0.0,
                        life: 1.0,
                        color: get_color_for_freq(x, num_bins),
                    });
                }
            }
        }

        let canvas = Canvas::default()
            .block(ratatui::widgets::Block::default().title(format!(" Style: {} ", self.name())).borders(ratatui::widgets::Borders::ALL))
            .x_bounds([0.0, num_bins as f64]).y_bounds([0.0, 50.0])
            .paint(|ctx| {
                for p in particles.iter() {
                    ctx.draw(&Points { coords: &[(p.x, p.y)], color: p.color });
                }
            });
        f.render_widget(canvas, area);
    }
}

// --- Implementation: Mixed ---

impl Visualizer for MixedParticles {
    fn name(&self) -> &str {
        "Particles: Chaos"
    }

    fn draw(&self, f: &mut Frame, area: Rect, spectrum: &FrequencySpectrum, beat_info: &BeatInfo) {
        let num_bins = 80;
        let bins = get_log_points(spectrum, num_bins);
        let mut particles = self.particles.lock().unwrap();

        for p in particles.iter_mut() {
            p.x += p.vx;
            p.y += p.vy;
            p.life -= 0.015;
            if beat_info.is_beat {
                p.x += p.vx * 2.0;
                p.y += p.vy * 2.0;
            }
        }
        particles.retain(|p| p.life > 0.0 && p.x >= 0.0 && p.x <= num_bins as f64 && p.y >= 0.0 && p.y <= 50.0);

        for (x, &val) in bins.iter().enumerate() {
            let freq_boost = 1.0 + (x as f32 / num_bins as f32) * 4.0;
            let adjusted_val = val * freq_boost;
            if adjusted_val > 0.01 {
                if random_range(0.0..1.0) < (adjusted_val * 12.0) as f64 {
                    particles.push(Particle {
                        x: x as f64,
                        y: 25.0,
                        vx: random_range(-1.0..1.0),
                        vy: random_range(-1.0..1.0),
                        life: 1.0,
                        color: get_color_for_freq(x, num_bins),
                    });
                }
            }
        }

        let canvas = Canvas::default()
            .block(ratatui::widgets::Block::default().title(format!(" Style: {} ", self.name())).borders(ratatui::widgets::Borders::ALL))
            .x_bounds([0.0, num_bins as f64]).y_bounds([0.0, 50.0])
            .paint(|ctx| {
                for p in particles.iter() {
                    ctx.draw(&Points { coords: &[(p.x, p.y)], color: p.color });
                }
            });
        f.render_widget(canvas, area);
    }
}
