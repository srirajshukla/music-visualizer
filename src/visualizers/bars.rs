use super::{BeatInfo, Visualizer};
use ratatui::{
    layout::Rect,
    style::Color,
    widgets::canvas::{Canvas, Line},
    Frame,
};
use spectrum_analyzer::FrequencySpectrum;
use std::sync::Mutex;

pub struct BarVisualizer {
    peaks: Mutex<Vec<f32>>,
}

impl BarVisualizer {
    pub fn new() -> Self {
        Self {
            peaks: Mutex::new(vec![0.0; 40]),
        }
    }

    fn get_log_bars(&self, spectrum: &FrequencySpectrum, num_bars: usize) -> Vec<f32> {
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
            .map(|(i, &v)| if counts[i] > 0 { v / counts[i] as f32 } else { 0.0 })
            .collect()
    }
}

impl Visualizer for BarVisualizer {
    fn name(&self) -> &str {
        "Enhanced Bars"
    }

    fn draw(&self, f: &mut Frame, area: Rect, spectrum: &FrequencySpectrum, beat_info: &BeatInfo) {
        let num_bars = 40;
        let heights = self.get_log_bars(spectrum, num_bars);
        let mut peaks = self.peaks.lock().unwrap();

        for i in 0..num_bars {
            let h = heights[i] * 300.0;
            if h > peaks[i] {
                peaks[i] = h;
            } else {
                peaks[i] = (peaks[i] - 0.5).max(0.0);
            }
        }

        let canvas = Canvas::default()
            .block(
                ratatui::widgets::Block::default()
                    .title(format!(" Style: {} ", self.name()))
                    .borders(ratatui::widgets::Borders::ALL)
                    .border_style(ratatui::style::Style::default().fg(if beat_info.is_beat {
                        Color::Yellow
                    } else {
                        Color::Green
                    })),
            )
            .x_bounds([0.0, num_bars as f64])
            .y_bounds([0.0, 50.0])
            .paint(|ctx| {
                let mid_y = 25.0;
                for i in 0..num_bars {
                    let h = (heights[i] * 300.0) as f64;
                    let x = i as f64 + 0.5;

                    let hue = i as f32 / num_bars as f32;
                    let color = if hue < 0.33 {
                        Color::Blue
                    } else if hue < 0.66 {
                        Color::Cyan
                    } else {
                        Color::White
                    };

                    ctx.draw(&Line {
                        x1: x,
                        y1: mid_y - h,
                        x2: x,
                        y2: mid_y + h,
                        color,
                    });

                    let peak_y = peaks[i] as f64;
                    ctx.draw(&Line {
                        x1: x - 0.2,
                        y1: mid_y + peak_y + 1.0,
                        x2: x + 0.2,
                        y2: mid_y + peak_y + 1.0,
                        color: Color::Red,
                    });
                    ctx.draw(&Line {
                        x1: x - 0.2,
                        y1: mid_y - peak_y - 1.0,
                        x2: x + 0.2,
                        y2: mid_y - peak_y - 1.0,
                        color: Color::Red,
                    });
                }
            });

        f.render_widget(canvas, area);
    }
}
