use super::{BeatInfo, Visualizer};
use ratatui::{
    layout::Rect,
    style::Color,
    widgets::canvas::{Canvas, Line},
    Frame,
};
use spectrum_analyzer::FrequencySpectrum;

pub struct LiquidVisualizer;

impl LiquidVisualizer {
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

impl Visualizer for LiquidVisualizer {
    fn name(&self) -> &str {
        "Liquid Mountains"
    }

    fn draw(&self, f: &mut Frame, area: Rect, spectrum: &FrequencySpectrum, beat_info: &BeatInfo) {
        let num_bins = 100;
        let bins = self.get_log_points(spectrum, num_bins);
        let color = if beat_info.is_beat { Color::Blue } else { Color::DarkGray };

        let canvas = Canvas::default()
            .block(
                ratatui::widgets::Block::default()
                    .title(format!(" Style: {} ", self.name()))
                    .borders(ratatui::widgets::Borders::ALL)
                    .border_style(ratatui::style::Style::default().fg(color)),
            )
            .x_bounds([0.0, num_bins as f64])
            .y_bounds([0.0, 50.0])
            .paint(|ctx| {
                for i in 0..bins.len().saturating_sub(1) {
                    let x1 = i as f64;
                    let y1 = (bins[i] * 500.0) as f64;
                    let x2 = (i + 1) as f64;
                    let y2 = (bins[i+1] * 500.0) as f64;
                    
                    // Fill vertical lines for "solid" look
                    ctx.draw(&Line { x1, y1: 0.0, x2: x1, y2: y1, color });
                    
                    // Top ridge
                    ctx.draw(&Line { x1, y1, x2, y2, color: Color::White });
                }
            });

        f.render_widget(canvas, area);
    }
}
