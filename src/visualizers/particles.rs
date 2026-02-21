use super::{BeatInfo, Visualizer};
use ratatui::{
    layout::Rect,
    style::Color,
    widgets::canvas::{Canvas, Points},
    Frame,
};
use spectrum_analyzer::FrequencySpectrum;

pub struct ParticleVisualizer;

impl ParticleVisualizer {
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
        let color = if beat_info.is_beat { Color::White } else { Color::Green };

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
                let mut points = Vec::new();
                for (x, &val) in bins.iter().enumerate() {
                    let h = (val * 400.0) as i32;
                    for y_off in 0..h {
                        if y_off % 2 == 0 {
                            points.push((x as f64, 25.0 + y_off as f64));
                            points.push((x as f64, 25.0 - y_off as f64));
                        }
                    }
                }
                ctx.draw(&Points { coords: &points, color });
            });

        f.render_widget(canvas, area);
    }
}
