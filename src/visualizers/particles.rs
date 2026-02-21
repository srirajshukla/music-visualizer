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
    vy: f64,
    life: f32,
    color: Color,
}

pub struct ParticleVisualizer {
    particles: Mutex<Vec<Particle>>,
}

impl ParticleVisualizer {
    pub fn new() -> Self {
        Self {
            particles: Mutex::new(Vec::with_capacity(200)),
        }
    }

    fn get_log_points(&self, spectrum: &FrequencySpectrum, num_bins: usize) -> Vec<f32> {
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
}

impl Visualizer for ParticleVisualizer {
    fn name(&self) -> &str {
        "Digital Particles"
    }

    fn draw(&self, f: &mut Frame, area: Rect, spectrum: &FrequencySpectrum, beat_info: &BeatInfo) {
        let num_bins = 80;
        let bins = self.get_log_points(spectrum, num_bins);
        let mut particles = self.particles.lock().unwrap();

        // 1. Update existing particles and filter out dead ones
        for p in particles.iter_mut() {
            p.y += p.vy;
            p.life -= 0.02;
            if beat_info.is_beat {
                p.y += p.vy * 2.0; // Speed up on beat
            }
        }
        particles.retain(|p| p.life > 0.0 && p.y > 0.0 && p.y < 50.0);

        // 2. Spawn new particles based on current spectrum
        for (x, &val) in bins.iter().enumerate() {
            // Frequency-dependent boost: High frequencies get more "gain" for spawning
            let freq_boost = 1.0 + (x as f32 / num_bins as f32) * 4.0;
            let adjusted_val = val * freq_boost;
            
            // Lower, more sensitive thresholds
            let threshold = if beat_info.is_beat { 0.005 } else { 0.01 };
            
            if adjusted_val > threshold {
                let hue = x as f32 / num_bins as f32;
                let color = if hue < 0.2 {
                    Color::Red
                } else if hue < 0.4 {
                    Color::Yellow
                } else if hue < 0.7 {
                    Color::Green
                } else if hue < 0.9 {
                    Color::Cyan
                } else {
                    Color::White
                };

                // Spawn column "rain" - using adjusted_val for higher sensitivity
                if random_range(0.0..1.0) < (adjusted_val * 8.0) as f64 {
                    particles.push(Particle {
                        x: x as f64,
                        y: 25.0 + (val * 100.0) as f64,
                        vy: -random_range(0.2..0.8),
                        life: 1.0,
                        color,
                    });
                    particles.push(Particle {
                        x: x as f64,
                        y: 25.0 - (val * 100.0) as f64,
                        vy: random_range(0.2..0.8),
                        life: 1.0,
                        color,
                    });
                }

                // Extra "Explosion" particles on beat
                if beat_info.is_beat && random_range(0.0..1.0) < (adjusted_val * 12.0) as f64 {
                    particles.push(Particle {
                        x: x as f64,
                        y: 25.0,
                        vy: random_range(-2.0..2.0),
                        life: 0.8,
                        color: Color::White,
                    });
                }
            }
        }

        let canvas = Canvas::default()
            .block(
                ratatui::widgets::Block::default()
                    .title(format!(" Style: {} ", self.name()))
                    .borders(ratatui::widgets::Borders::ALL)
                    .border_style(ratatui::style::Style::default().fg(if beat_info.is_beat { Color::White } else { Color::Green })),
            )
            .x_bounds([0.0, num_bins as f64])
            .y_bounds([0.0, 50.0])
            .paint(|ctx| {
                // Group points by color to reduce draw calls
                let mut color_groups: std::collections::HashMap<Color, Vec<(f64, f64)>> = std::collections::HashMap::new();
                for p in particles.iter() {
                    color_groups.entry(p.color).or_default().push((p.x, p.y));
                }

                for (color, coords) in color_groups {
                    ctx.draw(&Points { coords: &coords, color });
                }
            });

        f.render_widget(canvas, area);
    }
}
