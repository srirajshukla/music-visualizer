use super::{BeatInfo, Visualizer};
use ratatui::{
    layout::Rect,
    style::Color,
    widgets::canvas::{Canvas, Line},
    Frame,
};
use spectrum_analyzer::FrequencySpectrum;
use std::time::Instant;

/// Helper to extract frequency band energy
fn get_band_energy(spectrum: &FrequencySpectrum, min_f: f32, max_f: f32) -> f32 {
    let mut energy = 0.0;
    let mut count = 0;
    for (freq, val) in spectrum.to_map().iter() {
        let f = *freq as f32;
        if f >= min_f && f <= max_f {
            energy += val;
            count += 1;
        }
    }
    if count > 0 { energy / count as f32 } else { 0.0 }
}

// 1. --- Spectral Ribbons ---
pub struct SpectralRibbons {
    start_time: Instant,
}

impl SpectralRibbons {
    pub fn new() -> Self {
        Self { start_time: Instant::now() }
    }
}

impl Visualizer for SpectralRibbons {
    fn name(&self) -> &str { "Spectral Ribbons" }
    fn draw(&self, f: &mut Frame, area: Rect, spectrum: &FrequencySpectrum, _beat_info: &BeatInfo) {
        let elapsed = self.start_time.elapsed().as_secs_f32();
        
        // Define 5 frequency bands for more detail
        let sub_bass = get_band_energy(spectrum, 20.0, 60.0) * 600.0;
        let bass = get_band_energy(spectrum, 60.0, 250.0) * 500.0;
        let mids = get_band_energy(spectrum, 250.0, 2000.0) * 1000.0;
        let upper_mids = get_band_energy(spectrum, 2000.0, 6000.0) * 1500.0;
        let highs = get_band_energy(spectrum, 6000.0, 15000.0) * 3000.0;

        let canvas = Canvas::default()
            .block(ratatui::widgets::Block::default().title(format!(" Style: {} ", self.name())).borders(ratatui::widgets::Borders::ALL))
            .x_bounds([0.0, 100.0])
            .y_bounds([-40.0, 40.0])
            .paint(|ctx| {
                // Draw 5 ribbons with vertical offsets
                let ribbons = [
                    (sub_bass, Color::Magenta, 0.4, 0.8, -24.0), // Deep Sub
                    (bass, Color::Blue, 0.6, 1.2, -12.0),       // Bass
                    (mids, Color::Cyan, 1.2, 2.5, 0.0),          // Mids
                    (upper_mids, Color::Green, 2.2, 3.8, 12.0),  // Upper Mids
                    (highs, Color::White, 4.0, 6.0, 24.0),       // Highs
                ];

                for (amp, color, freq, speed, y_off) in ribbons {
                    let mut prev_x = 0.0;
                    let mut prev_y = y_off + (elapsed * speed).sin() * amp;
                    
                    for x in (1..=100).step_by(2) {
                        let x_f = x as f32;
                        // Multiple harmonics per ribbon for "flowing silk" effect
                        let wave1 = (x_f * 0.08 * freq + elapsed * speed).sin() * amp;
                        let wave2 = (x_f * 0.15 * freq - elapsed * speed * 0.7).cos() * (amp * 0.4);
                        let wave3 = (x_f * 0.3 * freq + elapsed * speed * 1.5).sin() * (amp * 0.15);
                        
                        let y = y_off + wave1 + wave2 + wave3;

                        ctx.draw(&Line {
                            x1: prev_x as f64,
                            y1: prev_y as f64,
                            x2: x_f as f64,
                            y2: y as f64,
                            color,
                        });
                        prev_x = x_f;
                        prev_y = y;
                    }
                }
            });
        f.render_widget(canvas, area);
    }
}

// 2. --- Lissajous Interference (Original) ---
pub struct LissajousInterference {
    start_time: Instant,
}

impl LissajousInterference {
    pub fn new() -> Self {
        Self { start_time: Instant::now() }
    }
}

impl Visualizer for LissajousInterference {
    fn name(&self) -> &str { "Lissajous: Original" }
    fn draw(&self, f: &mut Frame, area: Rect, spectrum: &FrequencySpectrum, beat_info: &BeatInfo) {
        let elapsed = self.start_time.elapsed().as_secs_f32();
        let bass = get_band_energy(spectrum, 20.0, 150.0) * 400.0;
        let highs = get_band_energy(spectrum, 2000.0, 10000.0) * 2000.0;

        let canvas = Canvas::default()
            .block(ratatui::widgets::Block::default().title(format!(" Style: {} ", self.name())).borders(ratatui::widgets::Borders::ALL))
            .x_bounds([-30.0, 30.0])
            .y_bounds([-30.0, 30.0])
            .paint(|ctx| {
                let mut prev_x = 0.0;
                let mut prev_y = 0.0;
                let freq_x = 2.0 + bass * 0.05;
                let freq_y = 3.0 + highs * 0.01;

                for t in 0..150 {
                    let t_f = t as f32 * 0.12;
                    let x = (t_f * freq_x + elapsed).sin() * 20.0;
                    let y = (t_f * freq_y + elapsed * 1.5).cos() * 20.0;
                    if t > 0 {
                        ctx.draw(&Line { x1: prev_x as f64, y1: prev_y as f64, x2: x as f64, y2: y as f64, color: if beat_info.is_beat { Color::Yellow } else { Color::Cyan } });
                    }
                    prev_x = x;
                    prev_y = y;
                }
            });
        f.render_widget(canvas, area);
    }
}

// 3. --- Lissajous: Enhanced (Mixed Version) ---
pub struct LissajousEnhanced {
    start_time: Instant,
}

impl LissajousEnhanced {
    pub fn new() -> Self {
        Self { start_time: Instant::now() }
    }
}

impl Visualizer for LissajousEnhanced {
    fn name(&self) -> &str { "Lissajous: Enhanced" }
    fn draw(&self, f: &mut Frame, area: Rect, spectrum: &FrequencySpectrum, beat_info: &BeatInfo) {
        let elapsed = self.start_time.elapsed().as_secs_f32();
        let bass = get_band_energy(spectrum, 20.0, 150.0) * 450.0;
        let highs = get_band_energy(spectrum, 2000.0, 10000.0) * 2500.0;
        
        let beat_scale = if beat_info.is_beat { 1.25 } else { 1.0 };
        let base_radius = 18.0 * beat_scale;

        let canvas = Canvas::default()
            .block(ratatui::widgets::Block::default().title(format!(" Style: {} ", self.name())).borders(ratatui::widgets::Borders::ALL))
            .x_bounds([-35.0, 35.0])
            .y_bounds([-35.0, 35.0])
            .paint(|ctx| {
                let freq_x = 2.0 + bass * 0.04;
                let freq_y = 3.0 + highs * 0.015;

                for i in (0..3).rev() {
                    let t_offset = i as f32 * 0.08;
                    let trail_elapsed = elapsed - t_offset;
                    
                    let trail_color = match i {
                        0 => if beat_info.is_beat { Color::White } else { Color::Cyan },
                        1 => Color::Blue,
                        _ => Color::DarkGray,
                    };

                    let mut prev_x = 0.0;
                    let mut prev_y = 0.0;

                    for t in 0..150 {
                        let t_f = t as f32 * 0.12;
                        let orbit = (t_f * 8.0 + trail_elapsed * 2.0).sin() * (highs * 0.08);
                        
                        let x = (t_f * freq_x + trail_elapsed).sin() * (base_radius + orbit);
                        let y = (t_f * freq_y + trail_elapsed * 1.3).cos() * (base_radius + orbit);
                        
                        if t > 0 {
                            let color = if i == 0 {
                                let dist = (x*x + y*y).sqrt();
                                if dist > 22.0 { Color::White }
                                else if dist > 15.0 { Color::Cyan }
                                else { Color::Blue }
                            } else {
                                trail_color
                            };

                            ctx.draw(&Line {
                                x1: prev_x as f64,
                                y1: prev_y as f64,
                                x2: x as f64,
                                y2: y as f64,
                                color,
                            });
                        }
                        prev_x = x;
                        prev_y = y;
                    }
                }
            });
        f.render_widget(canvas, area);
    }
}


// 4. --- Resonant Helix Ribbons (Hybrid) ---
pub struct ResonantHelix {
    start_time: Instant,
}

impl ResonantHelix {
    pub fn new() -> Self {
        Self { start_time: Instant::now() }
    }
}

impl Visualizer for ResonantHelix {
    fn name(&self) -> &str { "Resonant Helix" }
    fn draw(&self, f: &mut Frame, area: Rect, spectrum: &FrequencySpectrum, beat_info: &BeatInfo) {
        let elapsed = self.start_time.elapsed().as_secs_f32();
        let bass = get_band_energy(spectrum, 20.0, 150.0) * 600.0;
        let highs = get_band_energy(spectrum, 2000.0, 10000.0) * 3000.0;
        let beat_pulse = if beat_info.is_beat { 1.4 } else { 1.0 };

        let canvas = Canvas::default()
            .block(ratatui::widgets::Block::default().title(format!(" Style: {} ", self.name())).borders(ratatui::widgets::Borders::ALL))
            .x_bounds([0.0, 100.0])
            .y_bounds([-35.0, 35.0])
            .paint(|ctx| {
                // Pre-calculate Y positions for all 3 strands at once for "rung" connection
                let mut strands_y = vec![vec![0.0f32; 101]; 3];

                for i in 0..3 {
                    let offset = i as f32 * (std::f32::consts::PI * 2.0 / 3.0);
                    let color = match i {
                        0 => Color::Blue,
                        1 => Color::Cyan,
                        _ => Color::White,
                    };

                    let mut prev_x = 0.0;
                    
                    for x in (0..=100).step_by(2) {
                        let x_f = x as f32;
                        let twist = 0.12 + (highs * 0.04);
                        let base_phase = x_f * twist + elapsed * 3.5 + offset;
                        let ripple = (x_f * 0.6 + elapsed * 10.0).sin() * (highs * 0.2);
                        let y = base_phase.sin() * bass * beat_pulse + ripple;
                        
                        strands_y[i][x] = y;

                        if x > 0 {
                            ctx.draw(&Line {
                                x1: prev_x as f64,
                                y1: strands_y[i][x-2] as f64,
                                x2: x_f as f64,
                                y2: y as f64,
                                color,
                            });
                        }

                        // Add "Energy Cord" style rungs connecting the strands
                        if x % 8 == 0 {
                            let next_i = (i + 1) % 3;
                            // Draw connecting lattice lines between strands
                            ctx.draw(&Line {
                                x1: x_f as f64,
                                y1: y as f64,
                                x2: x_f as f64,
                                y2: strands_y[next_i][x] as f64, // Note: This uses previous i's value if not yet calculated, which is fine for visual sync
                                color: Color::DarkGray,
                            });
                        }
                        
                        prev_x = x_f;
                    }
                }
            });
        f.render_widget(canvas, area);
    }
}
