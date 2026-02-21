use super::{BeatInfo, Visualizer};
use ratatui::{
    layout::Rect,
    style::Color,
    widgets::canvas::{Canvas, Line},
    Frame,
};
use spectrum_analyzer::FrequencySpectrum;

pub struct WaveformVisualizer;

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

            let log_f = f.ln();
            let bin_idx = (((log_f - min_log) / log_range) * num_bins as f32) as usize;
            let bin_idx = bin_idx.min(num_bins - 1);

            bins[bin_idx] += val;
            counts[bin_idx] += 1;
        }

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
            let height = (*val * 200.0) as f64;
            top_points.push((x, mid_y + height));
            bottom_points.push((x, mid_y - height));
        }

        let canvas = Canvas::default()
            .block(
                ratatui::widgets::Block::default()
                    .title(format!(" Style: {} ", self.name()))
                    .borders(ratatui::widgets::Borders::ALL)
                    .border_style(ratatui::style::Style::default().fg(color)),
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
