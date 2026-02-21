use super::{BeatInfo, Visualizer};
use ratatui::{
    layout::Rect,
    style::Color,
    widgets::canvas::{Canvas, Line, Points},
    Frame,
};
use spectrum_analyzer::FrequencySpectrum;
use std::sync::Mutex;
use rand::random_range;

// --- Helper for Logarithmic Scaling ---
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

// --- Liquid World (The Combined Style) ---
pub struct LiquidWorld {
    mist: Mutex<Vec<(f64, f64, f64)>>,
    fog_offset: Mutex<f64>,
}

impl LiquidWorld {
    pub fn new() -> Self {
        let mist = (0..60).map(|_| (random_range(0.0..100.0), random_range(20.0..50.0), random_range(0.1..0.4))).collect();
        Self { 
            mist: Mutex::new(mist),
            fog_offset: Mutex::new(0.0),
        }
    }
}

impl Visualizer for LiquidWorld {
    fn name(&self) -> &str {
        "Liquid World"
    }

    fn draw(&self, f: &mut Frame, area: Rect, spectrum: &FrequencySpectrum, beat_info: &BeatInfo) {
        let num_bins = 100;
        let bins = get_log_points(spectrum, num_bins);
        
        let mut fog_offset = self.fog_offset.lock().unwrap();
        *fog_offset += 0.4;
        if *fog_offset > 100.0 { *fog_offset = 0.0; }
        let current_fog = *fog_offset;

        let mut mist = self.mist.lock().unwrap();
        for m in mist.iter_mut() {
            m.1 -= m.2;
            if beat_info.is_beat { m.1 += 2.5; }
            m.0 += random_range(-0.15..0.15);
            if m.1 < 0.0 { m.1 = 50.0; m.0 = random_range(0.0..100.0); }
            if m.1 > 50.0 { m.1 = 50.0; }
            if m.0 < 0.0 { m.0 = 100.0; }
            if m.0 > 100.0 { m.0 = 0.0; }
        }

        let canvas = Canvas::default()
            .block(ratatui::widgets::Block::default().title(format!(" Style: {} ", self.name())).borders(ratatui::widgets::Borders::ALL))
            .x_bounds([0.0, num_bins as f64]).y_bounds([0.0, 50.0])
            .paint(|ctx| {
                // 1. Draw Mist Sky
                let mist_coords: Vec<(f64, f64)> = mist.iter().map(|m| (m.0, m.1)).collect();
                ctx.draw(&Points { coords: &mist_coords, color: if beat_info.is_beat { Color::White } else { Color::DarkGray } });

                // 2. Back Mountain Layer
                for i in 0..num_bins.saturating_sub(1) {
                    let h1 = (bins[i] * 300.0) as f64;
                    let h2 = (bins[i+1] * 300.0) as f64;
                    ctx.draw(&Line { x1: i as f64, y1: 0.0, x2: i as f64, y2: h1, color: Color::Black });
                    ctx.draw(&Line { x1: i as f64, y1: h1, x2: (i+1) as f64, y2: h2, color: Color::DarkGray });
                }

                // 3. Middle Mountain Layer
                for i in 0..num_bins.saturating_sub(1) {
                    let h1 = (bins[i] * 450.0) as f64;
                    let h2 = (bins[i+1] * 450.0) as f64;
                    if h1 > 1.5 {
                        ctx.draw(&Line { x1: i as f64, y1: 0.0, x2: i as f64, y2: h1 * 0.5, color: Color::Black });
                        ctx.draw(&Line { x1: i as f64, y1: h1 * 0.5, x2: i as f64, y2: h1, color: Color::Blue });
                        ctx.draw(&Line { x1: i as f64, y1: h1, x2: (i+1) as f64, y2: h2, color: Color::Cyan });
                    }
                }

                // 4. Front Mountain Layer
                let front_color = if beat_info.is_beat { Color::Yellow } else { Color::White };
                for i in 0..num_bins.saturating_sub(1) {
                    let h1 = (bins[i] * 600.0) as f64;
                    let h2 = (bins[i+1] * 600.0) as f64;
                    if h1 > 3.0 {
                        ctx.draw(&Line { x1: i as f64, y1: h1, x2: (i+1) as f64, y2: h2, color: front_color });
                    }
                    if (i as f64 + current_fog) as i32 % 20 < 6 {
                         ctx.draw(&Points { coords: &[(i as f64, random_range(1.0..5.0))], color: Color::Gray });
                    }
                }
            });
        f.render_widget(canvas, area);
    }
}
