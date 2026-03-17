use ::rand::Rng;
use rodio::{OutputStream, OutputStreamHandle, Sink, Source};
use std::cell::RefCell;
use std::time::Duration;

pub struct Sfx {
    _stream: OutputStream,
    handle: OutputStreamHandle,
    drone: RefCell<Option<Sink>>,
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

/// Notes per octave in the scale (7 for modal, 5 for pentatonic, etc.)
fn notes_per_octave(scale: &[f32]) -> usize {
    (scale.len() / 2).max(1)
}

/// Pick a bass note biased toward root and fifth (scale degrees 1 and 5)
fn pick_bass(scale: &[f32], rng: &mut impl ::rand::Rng) -> f32 {
    if scale.is_empty() { return 220.0; }
    let npo = notes_per_octave(scale);
    // Strong bass tones: root(0), fifth(4 in 7-note, 3 in 5-note), octave root
    let fifth_idx = (npo * 4 / 7).min(npo - 1); // approximate fifth
    let candidates = [0, fifth_idx, npo]; // root, fifth, octave root
    let idx = candidates[rng.gen_range(0..candidates.len())];
    scale[idx.min(scale.len() - 1)] * 0.5 // one octave down for bass weight
}

/// Pick a dyad: root + third (2 scale degrees up)
fn pick_dyad(scale: &[f32], rng: &mut impl ::rand::Rng) -> Vec<f32> {
    if scale.len() < 3 { return vec![pick(scale, rng)]; }
    let max_root = scale.len() - 3;
    let root = rng.gen_range(0..=max_root);
    vec![scale[root], scale[root + 2]]
}

/// Pick a dyad from the low end of the scale
fn pick_dyad_low(scale: &[f32], rng: &mut impl ::rand::Rng) -> Vec<f32> {
    if scale.len() < 3 { return vec![pick_low(scale, rng)]; }
    let end = (scale.len() / 3).max(3);
    let max_root = end.saturating_sub(3);
    let root = rng.gen_range(0..=max_root);
    vec![scale[root], scale[root + 2]]
}

/// Pick a dyad from the high end of the scale
fn pick_dyad_high(scale: &[f32], rng: &mut impl ::rand::Rng) -> Vec<f32> {
    if scale.len() < 3 { return vec![pick_high(scale, rng)]; }
    let start = (scale.len() * 2 / 3).min(scale.len() - 3);
    let max_root = scale.len() - 3;
    let root = rng.gen_range(start..=max_root);
    vec![scale[root], scale[root + 2]]
}

/// Pick a triad: root + third + fifth (0, 2, 4 scale degrees up)
fn pick_triad(scale: &[f32], rng: &mut impl ::rand::Rng) -> Vec<f32> {
    if scale.len() < 5 { return pick_dyad(scale, rng); }
    let max_root = scale.len() - 5;
    let root = rng.gen_range(0..=max_root);
    vec![scale[root], scale[root + 2], scale[root + 4]]
}

impl Sfx {
    pub fn new() -> Option<Self> {
        let (stream, handle) = OutputStream::try_default().ok()?;
        Some(Self { _stream: stream, handle, drone: RefCell::new(None) })
    }

    fn play(&self, source: impl Source<Item = f32> + Send + 'static) {
        if let Ok(sink) = Sink::try_new(&self.handle) {
            sink.append(source);
            sink.detach();
        }
    }

    // ── Game sounds — all in the level's musical mode ──

    pub fn footstep(&self, scale: &[f32]) {
        // Bassline — root/fifth biased, low single notes
        let mut rng = ::rand::thread_rng();
        let dur = rng.gen_range(25..45);
        let vol = rng.gen_range(0.046..0.092);
        let freq = pick_bass(scale, &mut rng);
        self.play(
            Osc::sine(freq)
                .take_duration(Duration::from_millis(dur))
                .amplify(vol as f32)
                .fade_out(Duration::from_millis(dur))
        );
    }

    pub fn hit(&self, scale: &[f32]) {
        // Punch — low dyad, square wave
        let mut rng = ::rand::thread_rng();
        let freqs: Vec<f32> = pick_dyad_low(scale, &mut rng).iter().map(|f| f * 0.5).collect();
        self.play(
            Chord::square(&freqs)
                .take_duration(Duration::from_millis(80))
                .amplify(0.15)
                .fade_out(Duration::from_millis(80))
        );
    }

    pub fn crit(&self, scale: &[f32]) {
        // Hard slam — low triad, saw wave
        let mut rng = ::rand::thread_rng();
        let freqs: Vec<f32> = pick_triad(scale, &mut rng).iter().map(|f| f * 0.25).collect();
        self.play(
            Chord::saw(&freqs)
                .take_duration(Duration::from_millis(120))
                .amplify(0.25)
                .fade_out(Duration::from_millis(120))
        );
    }

    pub fn player_hurt(&self, scale: &[f32]) {
        // Descending dyad sweep
        let mut rng = ::rand::thread_rng();
        let hi = pick_dyad_high(scale, &mut rng);
        let lo = pick_dyad_low(scale, &mut rng);
        let hi_avg = hi.iter().sum::<f32>() / hi.len() as f32;
        let lo_avg = lo.iter().sum::<f32>() / lo.len() as f32;
        // Sweep the root, chord the destination
        self.play(
            Sweep::new(hi_avg, lo_avg, Duration::from_millis(100), Waveform::Square)
                .amplify(0.10)
                .fade_out(Duration::from_millis(100))
                .then(
                    Chord::square(&lo)
                        .take_duration(Duration::from_millis(60))
                        .amplify(0.12)
                        .fade_out(Duration::from_millis(60))
                )
        );
    }

    pub fn miss(&self, scale: &[f32]) {
        // Quick high dyad, very quiet
        let mut rng = ::rand::thread_rng();
        let freqs = pick_dyad_high(scale, &mut rng);
        self.play(
            Chord::sine(&freqs)
                .take_duration(Duration::from_millis(40))
                .amplify(0.03)
                .fade_out(Duration::from_millis(40))
        );
    }

    pub fn kill(&self, scale: &[f32]) {
        // Rising sweep into a triumphant dyad
        let mut rng = ::rand::thread_rng();
        let lo = pick_low(scale, &mut rng);
        let hi_chord = pick_dyad_high(scale, &mut rng);
        let hi_avg = hi_chord.iter().sum::<f32>() / hi_chord.len() as f32;
        self.play(
            Sweep::new(lo, hi_avg, Duration::from_millis(60), Waveform::Sine)
                .amplify(0.10)
                .then(
                    Chord::sine(&hi_chord)
                        .take_duration(Duration::from_millis(50))
                        .amplify(0.12)
                        .fade_out(Duration::from_millis(50))
                )
        );
    }

    pub fn death(&self, scale: &[f32]) {
        // Long descending crash into sustained low triad
        let mut rng = ::rand::thread_rng();
        let hi = pick_high(scale, &mut rng);
        let lo_chord: Vec<f32> = pick_triad(scale, &mut rng).iter().map(|f| f * 0.25).collect();
        let lo_avg = lo_chord.iter().sum::<f32>() / lo_chord.len() as f32;
        self.play(
            Sweep::new(hi, lo_avg, Duration::from_millis(800), Waveform::Saw)
                .amplify(0.15)
                .fade_out(Duration::from_millis(800))
                .then(
                    Chord::saw(&lo_chord)
                        .take_duration(Duration::from_millis(1200))
                        .amplify(0.12)
                        .fade_out(Duration::from_millis(1200))
                )
        );
    }

    pub fn victory(&self, scale: &[f32]) {
        // Ascending 3-chord fanfare — each note becomes a dyad
        let mut rng = ::rand::thread_rng();
        let notes = pick_ascending(scale, 5, &mut rng);
        let c0 = [notes[0], notes.get(2).copied().unwrap_or(notes[0])];
        let c1 = [notes.get(1).copied().unwrap_or(notes[0]), notes.get(3).copied().unwrap_or(notes[0])];
        let c2 = [notes.get(2).copied().unwrap_or(notes[0]), notes.get(4).copied().unwrap_or(notes[0])];
        self.play(
            Chord::sine(&c0).take_duration(Duration::from_millis(100)).amplify(0.12)
                .then(silence(Duration::from_millis(30)))
                .then(Chord::sine(&c1).take_duration(Duration::from_millis(100)).amplify(0.12))
                .then(silence(Duration::from_millis(30)))
                .then(Chord::sine(&c2).take_duration(Duration::from_millis(200)).amplify(0.15)
                    .fade_out(Duration::from_millis(200)))
        );
    }

    pub fn pickup_gold(&self, scale: &[f32]) {
        // High sparkle dyad
        let mut rng = ::rand::thread_rng();
        let freqs: Vec<f32> = pick_dyad_high(scale, &mut rng).iter().map(|f| f * 2.0).collect();
        self.play(
            Chord::sine(&freqs)
                .take_duration(Duration::from_millis(60))
                .amplify(0.10)
                .fade_out(Duration::from_millis(60))
        );
    }

    pub fn pickup_potion(&self, scale: &[f32]) {
        // Rising sweep into a dyad
        let mut rng = ::rand::thread_rng();
        let lo = pick_low(scale, &mut rng);
        let hi_chord = pick_dyad(scale, &mut rng);
        let hi_avg = hi_chord.iter().sum::<f32>() / hi_chord.len() as f32;
        self.play(
            Sweep::new(lo, hi_avg, Duration::from_millis(50), Waveform::Sine)
                .amplify(0.08)
                .then(
                    Chord::sine(&hi_chord)
                        .take_duration(Duration::from_millis(40))
                        .amplify(0.10)
                        .fade_out(Duration::from_millis(40))
                )
        );
    }

    pub fn pickup_weapon(&self, scale: &[f32]) {
        // Ascending 3-chord metallic fanfare
        let mut rng = ::rand::thread_rng();
        let notes = pick_ascending(scale, 5, &mut rng);
        let c0 = [notes[0], notes.get(2).copied().unwrap_or(notes[0])];
        let c1 = [notes.get(1).copied().unwrap_or(notes[0]), notes.get(3).copied().unwrap_or(notes[0])];
        let c2 = [notes.get(2).copied().unwrap_or(notes[0]), notes.get(4).copied().unwrap_or(notes[0])];
        self.play(
            Chord::saw(&c0).take_duration(Duration::from_millis(100)).amplify(0.10)
                .then(silence(Duration::from_millis(20)))
                .then(Chord::saw(&c1).take_duration(Duration::from_millis(100)).amplify(0.10))
                .then(silence(Duration::from_millis(20)))
                .then(Chord::saw(&c2).take_duration(Duration::from_millis(180)).amplify(0.12)
                    .fade_out(Duration::from_millis(180)))
        );
    }

    pub fn pickup_armor(&self, scale: &[f32]) {
        // Lower metallic dyad — saw wave
        let mut rng = ::rand::thread_rng();
        let freqs = pick_dyad_low(scale, &mut rng);
        self.play(
            Chord::saw(&freqs)
                .take_duration(Duration::from_millis(150))
                .amplify(0.10)
                .fade_out(Duration::from_millis(150))
        );
    }

    pub fn level_up(&self, scale: &[f32]) {
        // Ascending 4-chord arpeggio — each step is a dyad
        let mut rng = ::rand::thread_rng();
        let notes = pick_ascending(scale, 6, &mut rng);
        let c0 = [notes[0], notes.get(2).copied().unwrap_or(notes[0])];
        let c1 = [notes.get(1).copied().unwrap_or(notes[0]), notes.get(3).copied().unwrap_or(notes[0])];
        let c2 = [notes.get(2).copied().unwrap_or(notes[0]), notes.get(4).copied().unwrap_or(notes[0])];
        let c3 = [notes.get(3).copied().unwrap_or(notes[0]), notes.get(5).copied().unwrap_or(notes[0])];
        self.play(
            Chord::sine(&c0).take_duration(Duration::from_millis(80)).amplify(0.10)
                .then(Chord::sine(&c1).take_duration(Duration::from_millis(80)).amplify(0.10))
                .then(Chord::sine(&c2).take_duration(Duration::from_millis(80)).amplify(0.10))
                .then(Chord::sine(&c3).take_duration(Duration::from_millis(160)).amplify(0.13)
                    .fade_out(Duration::from_millis(160)))
        );
    }

    pub fn trap(&self, scale: &[f32]) {
        // Dissonant low chord stab
        let mut rng = ::rand::thread_rng();
        let freqs: Vec<f32> = pick_dyad_low(scale, &mut rng).iter().map(|f| f * 0.5).collect();
        let detuned: Vec<f32> = freqs.iter().map(|f| f * 0.9).collect();
        self.play(
            Chord::square(&freqs)
                .take_duration(Duration::from_millis(30))
                .amplify(0.15)
                .then(
                    Chord::square(&detuned)
                        .take_duration(Duration::from_millis(100))
                        .amplify(0.12)
                        .fade_out(Duration::from_millis(100))
                )
        );
    }

    pub fn boss_kill(&self, scale: &[f32]) {
        // Descending crash + ascending 4-chord fanfare
        let mut rng = ::rand::thread_rng();
        let lo = pick_low(scale, &mut rng) * 0.25;
        let hi = pick_high(scale, &mut rng);
        let notes = pick_ascending(scale, 6, &mut rng);
        let c0 = [notes[0], notes.get(2).copied().unwrap_or(notes[0])];
        let c1 = [notes.get(1).copied().unwrap_or(notes[0]), notes.get(3).copied().unwrap_or(notes[0])];
        let c2 = [notes.get(2).copied().unwrap_or(notes[0]), notes.get(4).copied().unwrap_or(notes[0])];
        let c3 = [notes.get(3).copied().unwrap_or(notes[0]), notes.get(5).copied().unwrap_or(notes[0])];
        self.play(
            Sweep::new(hi, lo, Duration::from_millis(300), Waveform::Saw)
                .amplify(0.18)
                .fade_out(Duration::from_millis(300))
                .then(silence(Duration::from_millis(100)))
                .then(Chord::sine(&c0).take_duration(Duration::from_millis(120)).amplify(0.12))
                .then(Chord::sine(&c1).take_duration(Duration::from_millis(120)).amplify(0.12))
                .then(Chord::sine(&c2).take_duration(Duration::from_millis(120)).amplify(0.12))
                .then(Chord::sine(&c3).take_duration(Duration::from_millis(300)).amplify(0.15)
                    .fade_out(Duration::from_millis(300)))
        );
    }

    /// Start a looping low drone for boss proximity. Call once when a level begins.
    pub fn start_boss_drone(&self, scale: &[f32]) {
        self.stop_boss_drone();
        let freq = if scale.is_empty() { 55.0 } else { scale[0] * 0.25 };
        // Detuned pair for unsettling beating
        let source = Chord::new(&[freq, freq * 1.02], Waveform::Sine);
        if let Ok(sink) = Sink::try_new(&self.handle) {
            sink.set_volume(0.0);
            sink.append(source);
            *self.drone.borrow_mut() = Some(sink);
        }
    }

    /// Update drone volume based on Euclidean distance to boss. Silent beyond 20 tiles.
    pub fn update_boss_drone(&self, distance: f32) {
        if let Some(sink) = self.drone.borrow().as_ref() {
            let max_dist: f32 = 20.0;
            if distance > max_dist {
                sink.set_volume(0.0);
            } else {
                let t = 1.0 - (distance / max_dist);
                sink.set_volume(t * 0.7);
            }
        }
    }

    pub fn stop_boss_drone(&self) {
        if let Some(sink) = self.drone.borrow_mut().take() {
            sink.stop();
        }
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

/// Multiple oscillators mixed together (chord/dyad), arpeggiated low-to-high
struct Chord {
    oscs: Vec<Osc>,
    sample_rate: u32,
    sample_idx: u64,
    stagger: u64,
}

impl Chord {
    fn new(freqs: &[f32], waveform: Waveform) -> Self {
        let mut sorted: Vec<f32> = freqs.to_vec();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        Self {
            oscs: sorted.iter().map(|&f| Osc::new(f, waveform)).collect(),
            sample_rate: 44100,
            sample_idx: 0,
            stagger: 441, // ~10ms between each note
        }
    }
    fn sine(freqs: &[f32]) -> Self { Self::new(freqs, Waveform::Sine) }
    fn square(freqs: &[f32]) -> Self { Self::new(freqs, Waveform::Square) }
    fn saw(freqs: &[f32]) -> Self { Self::new(freqs, Waveform::Saw) }
}

impl Iterator for Chord {
    type Item = f32;
    fn next(&mut self) -> Option<f32> {
        if self.oscs.is_empty() { return Some(0.0); }
        let n = self.oscs.len() as f32;
        let mut sum = 0.0f32;
        for (i, osc) in self.oscs.iter_mut().enumerate() {
            if self.sample_idx >= (i as u64) * self.stagger {
                if let Some(v) = osc.next() {
                    sum += v;
                }
            }
        }
        self.sample_idx += 1;
        Some(sum / n)
    }
}

impl Source for Chord {
    fn current_frame_len(&self) -> Option<usize> { None }
    fn channels(&self) -> u16 { 1 }
    fn sample_rate(&self) -> u32 { self.sample_rate }
    fn total_duration(&self) -> Option<Duration> { None }
}

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
