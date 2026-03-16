use ::rand::Rng;
use rodio::{OutputStream, OutputStreamHandle, Sink, Source};
use std::time::Duration;

pub struct Sfx {
    _stream: OutputStream,
    handle: OutputStreamHandle,
}

/// Pick a random note from the scale
fn pick(scale: &[f32], rng: &mut impl ::rand::Rng) -> f32 {
    if scale.is_empty() { 440.0 } else { scale[rng.gen_range(0..scale.len())] }
}

/// Pick from the lower third of the scale
fn pick_low(scale: &[f32], rng: &mut impl ::rand::Rng) -> f32 {
    if scale.is_empty() { return 220.0; }
    let end = (scale.len() / 3).max(1);
    scale[rng.gen_range(0..end)]
}

/// Pick from the upper third of the scale
fn pick_high(scale: &[f32], rng: &mut impl ::rand::Rng) -> f32 {
    if scale.is_empty() { return 880.0; }
    let start = (scale.len() * 2 / 3).min(scale.len() - 1);
    scale[rng.gen_range(start..scale.len())]
}

/// Pick N ascending notes from the scale (for arpeggios)
fn pick_ascending(scale: &[f32], n: usize, rng: &mut impl ::rand::Rng) -> Vec<f32> {
    if scale.is_empty() { return vec![440.0; n]; }
    let max_start = scale.len().saturating_sub(n);
    let start = rng.gen_range(0..=max_start);
    scale[start..start + n.min(scale.len() - start)].to_vec()
}

/// Pick N descending notes from the scale
fn pick_descending(scale: &[f32], n: usize, rng: &mut impl ::rand::Rng) -> Vec<f32> {
    let mut notes = pick_ascending(scale, n, rng);
    notes.reverse();
    notes
}

impl Sfx {
    pub fn new() -> Option<Self> {
        let (stream, handle) = OutputStream::try_default().ok()?;
        Some(Self { _stream: stream, handle })
    }

    fn play(&self, source: impl Source<Item = f32> + Send + 'static) {
        if let Ok(sink) = Sink::try_new(&self.handle) {
            sink.append(source);
            sink.detach();
        }
    }

    // ── Game sounds — all in the level's musical mode ──

    pub fn footstep(&self, scale: &[f32]) {
        let mut rng = ::rand::thread_rng();
        let dur = rng.gen_range(20..40);
        let vol = rng.gen_range(0.03..0.07);
        let freq = pick(scale, &mut rng);
        self.play(
            Osc::sine(freq)
                .take_duration(Duration::from_millis(dur))
                .amplify(vol as f32)
                .fade_out(Duration::from_millis(dur))
        );
    }

    pub fn hit(&self, scale: &[f32]) {
        // Punch — low note, square wave
        let mut rng = ::rand::thread_rng();
        let freq = pick_low(scale, &mut rng) * 0.5; // one octave below low range
        self.play(
            Osc::square(freq)
                .take_duration(Duration::from_millis(80))
                .amplify(0.15)
                .fade_out(Duration::from_millis(80))
        );
    }

    pub fn crit(&self, scale: &[f32]) {
        // Hard slam — lowest note, saw wave
        let mut rng = ::rand::thread_rng();
        let freq = pick_low(scale, &mut rng) * 0.25;
        self.play(
            Osc::saw(freq)
                .take_duration(Duration::from_millis(120))
                .amplify(0.25)
                .fade_out(Duration::from_millis(120))
        );
    }

    pub fn player_hurt(&self, scale: &[f32]) {
        // Descending two notes
        let mut rng = ::rand::thread_rng();
        let notes = pick_descending(scale, 2, &mut rng);
        let hi = notes.first().copied().unwrap_or(400.0);
        let lo = notes.last().copied().unwrap_or(200.0);
        self.play(
            Sweep::new(hi, lo, Duration::from_millis(150), Waveform::Square)
                .amplify(0.12)
                .fade_out(Duration::from_millis(150))
        );
    }

    pub fn miss(&self, scale: &[f32]) {
        // Quick high note, very quiet
        let mut rng = ::rand::thread_rng();
        let freq = pick_high(scale, &mut rng);
        self.play(
            Osc::sine(freq)
                .take_duration(Duration::from_millis(40))
                .amplify(0.03)
                .fade_out(Duration::from_millis(40))
        );
    }

    pub fn kill(&self, scale: &[f32]) {
        // Rising two-note pop
        let mut rng = ::rand::thread_rng();
        let notes = pick_ascending(scale, 2, &mut rng);
        let lo = notes.first().copied().unwrap_or(300.0);
        let hi = notes.last().copied().unwrap_or(600.0);
        self.play(
            Sweep::new(lo, hi, Duration::from_millis(100), Waveform::Sine)
                .amplify(0.12)
                .fade_out(Duration::from_millis(100))
        );
    }

    pub fn death(&self, scale: &[f32]) {
        // Long descending from high to lowest note
        let mut rng = ::rand::thread_rng();
        let hi = pick_high(scale, &mut rng);
        let lo = pick_low(scale, &mut rng) * 0.25;
        self.play(
            Sweep::new(hi, lo, Duration::from_millis(800), Waveform::Saw)
                .amplify(0.15)
                .fade_out(Duration::from_millis(800))
        );
    }

    pub fn victory(&self, scale: &[f32]) {
        // Ascending 3-note fanfare from the scale
        let mut rng = ::rand::thread_rng();
        let notes = pick_ascending(scale, 3, &mut rng);
        let n0 = notes.first().copied().unwrap_or(440.0);
        let n1 = notes.get(1).copied().unwrap_or(554.0);
        let n2 = notes.last().copied().unwrap_or(659.0);
        self.play(
            Osc::sine(n0).take_duration(Duration::from_millis(100)).amplify(0.12)
                .then(silence(Duration::from_millis(30)))
                .then(Osc::sine(n1).take_duration(Duration::from_millis(100)).amplify(0.12))
                .then(silence(Duration::from_millis(30)))
                .then(Osc::sine(n2).take_duration(Duration::from_millis(200)).amplify(0.15)
                    .fade_out(Duration::from_millis(200)))
        );
    }

    pub fn pickup_gold(&self, scale: &[f32]) {
        // High ding — highest note in scale
        let mut rng = ::rand::thread_rng();
        let freq = pick_high(scale, &mut rng) * 2.0; // octave up for sparkle
        self.play(
            Osc::sine(freq)
                .take_duration(Duration::from_millis(60))
                .amplify(0.10)
                .fade_out(Duration::from_millis(60))
        );
    }

    pub fn pickup_potion(&self, scale: &[f32]) {
        // Rising blip — two ascending notes
        let mut rng = ::rand::thread_rng();
        let notes = pick_ascending(scale, 2, &mut rng);
        let lo = notes.first().copied().unwrap_or(400.0);
        let hi = notes.last().copied().unwrap_or(800.0);
        self.play(
            Sweep::new(lo, hi, Duration::from_millis(80), Waveform::Sine)
                .amplify(0.10)
                .fade_out(Duration::from_millis(80))
        );
    }

    pub fn pickup_weapon(&self, scale: &[f32]) {
        // Metallic ring — mid note, saw wave
        let mut rng = ::rand::thread_rng();
        let freq = pick(scale, &mut rng);
        self.play(
            Osc::saw(freq)
                .take_duration(Duration::from_millis(120))
                .amplify(0.10)
                .fade_out(Duration::from_millis(120))
        );
    }

    pub fn pickup_armor(&self, scale: &[f32]) {
        // Lower metallic — low note, saw wave
        let mut rng = ::rand::thread_rng();
        let freq = pick_low(scale, &mut rng);
        self.play(
            Osc::saw(freq)
                .take_duration(Duration::from_millis(150))
                .amplify(0.10)
                .fade_out(Duration::from_millis(150))
        );
    }

    pub fn level_up(&self, scale: &[f32]) {
        // Ascending 4-note arpeggio
        let mut rng = ::rand::thread_rng();
        let notes = pick_ascending(scale, 4, &mut rng);
        let n0 = notes.first().copied().unwrap_or(330.0);
        let n1 = notes.get(1).copied().unwrap_or(440.0);
        let n2 = notes.get(2).copied().unwrap_or(550.0);
        let n3 = notes.last().copied().unwrap_or(660.0);
        self.play(
            Osc::sine(n0).take_duration(Duration::from_millis(80)).amplify(0.10)
                .then(Osc::sine(n1).take_duration(Duration::from_millis(80)).amplify(0.10))
                .then(Osc::sine(n2).take_duration(Duration::from_millis(80)).amplify(0.10))
                .then(Osc::sine(n3).take_duration(Duration::from_millis(160)).amplify(0.13)
                    .fade_out(Duration::from_millis(160)))
        );
    }

    pub fn trap(&self, scale: &[f32]) {
        // Dissonant low stab
        let mut rng = ::rand::thread_rng();
        let freq = pick_low(scale, &mut rng) * 0.5;
        self.play(
            Osc::square(freq)
                .take_duration(Duration::from_millis(30))
                .amplify(0.15)
                .then(
                    Osc::square(freq * 0.9) // slightly detuned for nastiness
                        .take_duration(Duration::from_millis(100))
                        .amplify(0.12)
                        .fade_out(Duration::from_millis(100))
                )
        );
    }

    pub fn boss_kill(&self, scale: &[f32]) {
        // Descending crash + ascending 4-note fanfare
        let mut rng = ::rand::thread_rng();
        let lo = pick_low(scale, &mut rng) * 0.25;
        let hi = pick_high(scale, &mut rng);
        let fanfare = pick_ascending(scale, 4, &mut rng);
        let f0 = fanfare.first().copied().unwrap_or(440.0);
        let f1 = fanfare.get(1).copied().unwrap_or(554.0);
        let f2 = fanfare.get(2).copied().unwrap_or(659.0);
        let f3 = fanfare.last().copied().unwrap_or(880.0);
        self.play(
            Sweep::new(hi, lo, Duration::from_millis(300), Waveform::Saw)
                .amplify(0.18)
                .fade_out(Duration::from_millis(300))
                .then(silence(Duration::from_millis(100)))
                .then(Osc::sine(f0).take_duration(Duration::from_millis(120)).amplify(0.12))
                .then(Osc::sine(f1).take_duration(Duration::from_millis(120)).amplify(0.12))
                .then(Osc::sine(f2).take_duration(Duration::from_millis(120)).amplify(0.12))
                .then(Osc::sine(f3).take_duration(Duration::from_millis(300)).amplify(0.15)
                    .fade_out(Duration::from_millis(300)))
        );
    }

    pub fn navigate(&self) {
        // UI tick — not tied to level scale
        self.play(
            Osc::sine(800.0)
                .take_duration(Duration::from_millis(30))
                .amplify(0.06)
                .fade_out(Duration::from_millis(30))
        );
    }

    pub fn confirm(&self) {
        // UI confirm — not tied to level scale
        self.play(
            Osc::sine(600.0).take_duration(Duration::from_millis(50)).amplify(0.08)
                .then(Osc::sine(900.0).take_duration(Duration::from_millis(80)).amplify(0.08)
                    .fade_out(Duration::from_millis(80)))
        );
    }
}

// ── Synth primitives ──

#[derive(Clone, Copy)]
enum Waveform { Sine, Square, Saw }

struct Osc {
    freq: f32,
    sample_rate: u32,
    phase: f32,
    waveform: Waveform,
}

impl Osc {
    fn new(freq: f32, waveform: Waveform) -> Self {
        Self { freq, sample_rate: 44100, phase: 0.0, waveform }
    }
    fn sine(freq: f32) -> Self { Self::new(freq, Waveform::Sine) }
    fn square(freq: f32) -> Self { Self::new(freq, Waveform::Square) }
    fn saw(freq: f32) -> Self { Self::new(freq, Waveform::Saw) }
}

impl Iterator for Osc {
    type Item = f32;
    fn next(&mut self) -> Option<f32> {
        let val = match self.waveform {
            Waveform::Sine => (self.phase * std::f32::consts::TAU).sin(),
            Waveform::Square => if self.phase < 0.5 { 1.0 } else { -1.0 },
            Waveform::Saw => 2.0 * self.phase - 1.0,
        };
        self.phase = (self.phase + self.freq / self.sample_rate as f32) % 1.0;
        Some(val)
    }
}

impl Source for Osc {
    fn current_frame_len(&self) -> Option<usize> { None }
    fn channels(&self) -> u16 { 1 }
    fn sample_rate(&self) -> u32 { self.sample_rate }
    fn total_duration(&self) -> Option<Duration> { None }
}

struct Sweep {
    start_freq: f32,
    end_freq: f32,
    duration: Duration,
    sample_rate: u32,
    sample_idx: u64,
    phase: f32,
    waveform: Waveform,
}

impl Sweep {
    fn new(start: f32, end: f32, duration: Duration, waveform: Waveform) -> Self {
        Self {
            start_freq: start, end_freq: end, duration,
            sample_rate: 44100, sample_idx: 0, phase: 0.0, waveform,
        }
    }
}

impl Iterator for Sweep {
    type Item = f32;
    fn next(&mut self) -> Option<f32> {
        let total = self.duration.as_secs_f32() * self.sample_rate as f32;
        if self.sample_idx as f32 >= total { return None; }
        let t = self.sample_idx as f32 / total;
        let freq = self.start_freq + (self.end_freq - self.start_freq) * t;
        let val = match self.waveform {
            Waveform::Sine => (self.phase * std::f32::consts::TAU).sin(),
            Waveform::Square => if self.phase < 0.5 { 1.0 } else { -1.0 },
            Waveform::Saw => 2.0 * self.phase - 1.0,
        };
        self.phase = (self.phase + freq / self.sample_rate as f32) % 1.0;
        self.sample_idx += 1;
        Some(val)
    }
}

impl Source for Sweep {
    fn current_frame_len(&self) -> Option<usize> { None }
    fn channels(&self) -> u16 { 1 }
    fn sample_rate(&self) -> u32 { self.sample_rate }
    fn total_duration(&self) -> Option<Duration> { Some(self.duration) }
}

fn silence(duration: Duration) -> rodio::source::Amplify<rodio::source::TakeDuration<Osc>> {
    Osc::sine(0.0).take_duration(duration).amplify(0.0)
}

trait FadeOutExt: Source<Item = f32> + Sized {
    fn fade_out(self, duration: Duration) -> FadeOut<Self>;
}

impl<S: Source<Item = f32>> FadeOutExt for S {
    fn fade_out(self, duration: Duration) -> FadeOut<Self> {
        let total = (duration.as_secs_f32() * self.sample_rate() as f32) as u64;
        FadeOut { source: self, total_samples: total, sample_idx: 0 }
    }
}

struct FadeOut<S> {
    source: S,
    total_samples: u64,
    sample_idx: u64,
}

impl<S: Source<Item = f32>> Iterator for FadeOut<S> {
    type Item = f32;
    fn next(&mut self) -> Option<f32> {
        let val = self.source.next()?;
        let t = (self.sample_idx as f32 / self.total_samples.max(1) as f32).min(1.0);
        let envelope = 1.0 - t;
        self.sample_idx += 1;
        Some(val * envelope)
    }
}

impl<S: Source<Item = f32>> Source for FadeOut<S> {
    fn current_frame_len(&self) -> Option<usize> { self.source.current_frame_len() }
    fn channels(&self) -> u16 { self.source.channels() }
    fn sample_rate(&self) -> u32 { self.source.sample_rate() }
    fn total_duration(&self) -> Option<Duration> { self.source.total_duration() }
}

trait ThenExt: Source<Item = f32> + Sized + Send + 'static {
    fn then<S: Source<Item = f32> + Send + 'static>(self, other: S) -> Box<dyn Source<Item = f32> + Send>;
}

impl<T: Source<Item = f32> + Send + 'static> ThenExt for T {
    fn then<S: Source<Item = f32> + Send + 'static>(self, other: S) -> Box<dyn Source<Item = f32> + Send> {
        Box::new(ChainSource { a: Some(Box::new(self)), b: Some(Box::new(other)) })
    }
}

struct ChainSource {
    a: Option<Box<dyn Source<Item = f32> + Send>>,
    b: Option<Box<dyn Source<Item = f32> + Send>>,
}

impl Iterator for ChainSource {
    type Item = f32;
    fn next(&mut self) -> Option<f32> {
        if let Some(a) = &mut self.a {
            match a.next() {
                Some(v) => return Some(v),
                None => { self.a = None; }
            }
        }
        if let Some(b) = &mut self.b {
            return b.next();
        }
        None
    }
}

impl Source for ChainSource {
    fn current_frame_len(&self) -> Option<usize> { None }
    fn channels(&self) -> u16 { 1 }
    fn sample_rate(&self) -> u32 { 44100 }
    fn total_duration(&self) -> Option<Duration> { None }
}
