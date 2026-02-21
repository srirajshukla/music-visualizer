use super::{BeatInfo, Visualizer};
use ratatui::{
    layout::Rect,
    style::Color,
    widgets::canvas::{Canvas, Line, Points},
    Frame,
};
use spectrum_analyzer::FrequencySpectrum;
use std::f64::consts::PI;
use std::sync::Mutex;
use rand::random_range;

struct Star {
    x: f64,
    y: f64,
    vx: f64,
    vy: f64,
    _brightness: f32,
}

pub struct RadialVisualizer {
    rotation: Mutex<f64>,
    stars: Mutex<Vec<Star>>,
    core_sides: Mutex<usize>,
}

impl RadialVisualizer {
    pub fn new() -> Self {
        let stars = (0..60)
            .map(|_| {
                let angle = random_range(0.0..2.0 * PI);
                let speed = random_range(0.1..0.5);
                Star {
                    x: random_range(-50.0..50.0),
                    y: random_range(-50.0..50.0),
                    vx: angle.cos() * speed,
                    vy: angle.sin() * speed,
                    _brightness: random_range(0.3..0.8),
                }
            })
            .collect();

        Self {
            rotation: Mutex::new(0.0),
            stars: Mutex::new(stars),
            core_sides: Mutex::new(30),
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

impl Visualizer for RadialVisualizer {
    fn name(&self) -> &str {
        "Radial Orbit"
    }

    fn draw(&self, f: &mut Frame, area: Rect, spectrum: &FrequencySpectrum, beat_info: &BeatInfo) {
        let mut rotation = self.rotation.lock().unwrap();
        let mut stars = self.stars.lock().unwrap();
        let mut core_sides = self.core_sides.lock().unwrap();

        // 1. Update State
        *rotation += 0.015;
        if beat_info.is_beat {
            *rotation += 0.08;
            *core_sides = match random_range(0..3) {
                0 => 3, // Triangle
                1 => 4, // Square
                _ => 6, // Hexagon
            };
        } else if *core_sides < 30 {
            *core_sides += 1; // Smoothly return to circle
        }

        for star in stars.iter_mut() {
            star.x += star.vx;
            star.y += star.vy;
            if beat_info.is_beat {
                 star.x += star.vx * 5.0;
                 star.y += star.vy * 5.0;
            }
            // Reset stars that go off screen
            if star.x.abs() > 60.0 || star.y.abs() > 60.0 {
                star.x = 0.0;
                star.y = 0.0;
            }
        }

        let current_rotation = *rotation;
        let num_bins = 60;
        let bins = self.get_log_points(spectrum, num_bins);

        let canvas = Canvas::default()
            .block(
                ratatui::widgets::Block::default()
                    .title(format!(" Style: {} ", self.name()))
                    .borders(ratatui::widgets::Borders::ALL)
                    .border_style(ratatui::style::Style::default().fg(if beat_info.is_beat { Color::LightRed } else { Color::Blue })),
            )
            .x_bounds([-60.0, 60.0])
            .y_bounds([-60.0, 60.0])
            .paint(|ctx| {
                // 2. Draw Nebula (Stars)
                let star_points: Vec<(f64, f64)> = stars.iter().map(|s| (s.x, s.y)).collect();
                ctx.draw(&Points {
                    coords: &star_points,
                    color: if beat_info.is_beat { Color::White } else { Color::DarkGray },
                });

                // 3. Draw Morphing Bass Core
                let bass_energy = bins.iter().take(10).sum::<f32>() / 10.0;
                let core_radius = 6.0 + (bass_energy * 60.0) as f64;
                let sides = *core_sides;
                for i in 0..sides {
                    let angle1 = (i as f64 / sides as f64) * 2.0 * PI + current_rotation * 0.5;
                    let angle2 = ((i + 1) as f64 / sides as f64) * 2.0 * PI + current_rotation * 0.5;
                    ctx.draw(&Line {
                        x1: angle1.cos() * core_radius,
                        y1: angle1.sin() * core_radius,
                        x2: angle2.cos() * core_radius,
                        y2: angle2.sin() * core_radius,
                        color: Color::Magenta,
                    });
                }

                // 4. Draw Counter-Rotating Rings (Bass, Mid, High)
                let ring_configs = [
                    (0..20, 18.0, 1.0, Color::Cyan),      // Bass Ring (Clockwise)
                    (20..40, 25.0, -1.2, Color::LightBlue), // Mid Ring (Counter-Clockwise)
                    (40..60, 32.0, 1.5, Color::Blue),      // High Ring (Fast Clockwise)
                ];

                for (range, base_radius, speed_mult, color) in ring_configs {
                    let ring_rotation = current_rotation * speed_mult;
                    let range_start = range.start;
                    let range_len = range.end - range.start;
                    
                    for i in range {
                        let idx_in_ring = i - range_start; 
                        let angle = (idx_in_ring as f64 / range_len as f64) * 2.0 * PI + ring_rotation;
                        let strength = (bins[i] * 200.0) as f64;
                        
                        let x1 = angle.cos() * base_radius;
                        let y1 = angle.sin() * base_radius;
                        let x2 = angle.cos() * (base_radius + strength);
                        let y2 = angle.sin() * (base_radius + strength);

                        ctx.draw(&Line { x1, y1, x2, y2, color });
                    }
                }
            });

        f.render_widget(canvas, area);
    }
}
