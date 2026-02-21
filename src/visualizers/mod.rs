use ratatui::{layout::Rect, Frame};
use spectrum_analyzer::FrequencySpectrum;

pub mod waveform;
pub mod bars;
pub mod radial;
pub mod particles;
pub mod liquid;
pub mod waves;

pub struct BeatInfo {
    pub is_beat: bool,
    pub bpm: f32,
    pub total_beats: usize,
}

pub trait Visualizer: Send + Sync {
    fn name(&self) -> &str;
    fn draw(&self, f: &mut Frame, area: Rect, spectrum: &FrequencySpectrum, beat_info: &BeatInfo);
}
