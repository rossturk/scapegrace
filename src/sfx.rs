use ::rand::Rng;
use rodio::{OutputStream, OutputStreamHandle, Sink, Source};
use std::cell::RefCell;
use std::time::Duration;

pub struct Sfx {
    _stream: OutputStream,
    handle: OutputStreamHandle,
    drone: RefCell<Option<Sink>>,
    bass_pos: RefCell<usize>,
    beat: RefCell<u32>,       // rhythmic pulse counter
    reverb: RefCell<ReverbParams>,  // current room acoustics
    reverb_enabled: RefCell<bool>,
    smooth_enabled: RefCell<bool>,
}

/// Reverb parameters derived from the space around the player.
#[derive(Clone, Copy)]
struct ReverbParams {
    delay_ms: f32,   // echo delay (big room ~40-80ms, hallway ~8-18ms)
    feedback: f32,   // decay amount 0..1 (big room ~0.5, hallway ~0.2)
    wet: f32,        // wet/dry mix 0..1 (big room ~0.35, hallway ~0.12)
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

/// Build a pentatonic scale from the mode by picking 5 of 7 notes (drop the 4th and 7th degrees).
/// This is compatible with all 7-note modes. One octave down for bass use.
fn pentatonic_bass(scale: &[f32]) -> Vec<f32> {
    if scale.is_empty() { return vec![220.0]; }
    let npo = notes_per_octave(scale);
    if npo < 7 {
        // Not a 7-note scale, use as-is
        return scale.iter().map(|f| f * 0.5).collect();
    }
    // Pick degrees 0, 1, 2, 4, 5 (drop 3rd and 6th — the "avoid" notes)
    [0, 1, 2, 4, 5].iter()
        .map(|&d| scale[d.min(scale.len() - 1)] * 0.5)
        .collect()
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

/// Pick a diatonic minor triad from the scale.
/// Finds root degrees where root+2 is a minor third (~3 semitones) and root+4 is a perfect fifth (~7 semitones).
fn pick_minor_triad(scale: &[f32], rng: &mut impl ::rand::Rng) -> Vec<f32> {
    if scale.len() < 5 { return pick_dyad(scale, rng); }
    let minor_third = 2.0_f32.powf(3.0 / 12.0); // ~1.189
    let perfect_fifth = 2.0_f32.powf(7.0 / 12.0); // ~1.498
    let tolerance = 0.02;
    let mut candidates: Vec<usize> = Vec::new();
    for i in 0..scale.len() - 4 {
        let third_ratio = scale[i + 2] / scale[i];
        let fifth_ratio = scale[i + 4] / scale[i];
        if (third_ratio - minor_third).abs() < tolerance
            && (fifth_ratio - perfect_fifth).abs() < tolerance
        {
            candidates.push(i);
        }
    }
    if candidates.is_empty() {
        // Fallback to any diatonic triad
        return pick_triad(scale, rng);
    }
    let root = candidates[rng.gen_range(0..candidates.len())];
    vec![scale[root], scale[root + 2], scale[root + 4]]
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
        Some(Self {
            _stream: stream, handle,
            drone: RefCell::new(None),
            bass_pos: RefCell::new(0), beat: RefCell::new(0),
            reverb: RefCell::new(ReverbParams { delay_ms: 20.0, feedback: 0.3, wet: 0.15 }),
            reverb_enabled: RefCell::new(true),
            smooth_enabled: RefCell::new(true),
        })
    }

    fn play(&self, source: impl Source<Item = f32> + Send + 'static) {
        let do_smooth = *self.smooth_enabled.borrow();
        let do_reverb = *self.reverb_enabled.borrow();
        let smoothed: Box<dyn Source<Item = f32> + Send> = if do_smooth {
            Box::new(source.smooth(5))
        } else {
            Box::new(source)
        };
        let processed: Box<dyn Source<Item = f32> + Send> = if do_reverb {
            let r = *self.reverb.borrow();
            Box::new(Reverb::new(smoothed, r.delay_ms, r.feedback, r.wet))
        } else {
            smoothed
        };

        let samples: Vec<f32> = processed.collect();
        let buf = rodio::buffer::SamplesBuffer::new(1, 44100, samples);
        let _ = self.handle.play_raw(buf.convert_samples());
    }

    pub fn set_reverb_enabled(&self, on: bool) { *self.reverb_enabled.borrow_mut() = on; }
    pub fn set_smooth_enabled(&self, on: bool) { *self.smooth_enabled.borrow_mut() = on; }
    pub fn reverb_params(&self) -> (f32, f32, f32) {
        let r = *self.reverb.borrow();
        (r.delay_ms, r.feedback, r.wet)
    }

    /// Update reverb based on how open the space is around the player.
    /// `openness` ranges from 0.0 (tight corridor) to 1.0 (huge open room).
    /// Call this whenever the player moves.
    pub fn update_room_acoustics(&self, openness: f32) {
        let o = openness.clamp(0.0, 1.0);
        *self.reverb.borrow_mut() = ReverbParams {
            delay_ms: 12.0 + o * 80.0,      // 12ms (corridor) → 92ms (cavern)
            feedback: 0.25 + o * 0.45,      // 0.25 (tight) → 0.70 (cavernous)
            wet: 0.40 + o * 0.30,           // 0.40 (corridor) → 0.70 (washy cavern)
        };
    }

    // ── Game sounds — all in the level's musical mode ──

    pub fn footstep(&self, scale: &[f32]) {
        // Walking bassline with varied rhythm — swing, ghost notes, rests, accents
        let mut rng = ::rand::thread_rng();
        let penta = pentatonic_bass(scale);
        if penta.is_empty() { return; }

        let mut beat = *self.beat.borrow();
        beat = beat.wrapping_add(1);
        *self.beat.borrow_mut() = beat;

        let mut pos = *self.bass_pos.borrow();

        // Rhythm pattern based on beat position in a 4-beat cycle
        let beat_in_bar = beat % 8;

        // Occasional rest — skip the note entirely (ghost of silence)
        if beat_in_bar == 7 && rng.gen_range(0..3) == 0 {
            return; // rest on the "and of 4" sometimes
        }

        // Movement: more interesting patterns based on rhythmic position
        let step: i32 = match beat_in_bar {
            0 => 0,        // downbeat: often repeat (anchor)
            1 | 5 => match rng.gen_range(0..4) {
                0..=2 => 1, _ => -1  // off-beats: mostly step up
            },
            2 | 6 => match rng.gen_range(0..6) {
                0..=2 => 1, 3..=4 => -1, _ => 2  // mid-bar: allow leaps
            },
            3 => match rng.gen_range(0..3) {
                0 => -2, 1 => -1, _ => 1  // approach the downbeat from below
            },
            4 => 1,        // beat 3: walk up
            _ => match rng.gen_range(0..10) {
                0..=4 => 1, 5..=7 => -1, 8 => 2, _ => 0,
            },
        };
        pos = ((pos as i32 + step).rem_euclid(penta.len() as i32)) as usize;
        *self.bass_pos.borrow_mut() = pos;

        let freq = penta[pos];

        // Swing feel: alternate long-short durations
        let is_downbeat = beat_in_bar % 2 == 0;
        let base_dur: u64 = if is_downbeat {
            rng.gen_range(55..80)  // longer on downbeats
        } else {
            rng.gen_range(30..50)  // shorter on upbeats
        };

        // Ghost notes: occasional very quiet, very short notes
        let is_ghost = !is_downbeat && rng.gen_range(0..4) == 0;
        let (dur, vol) = if is_ghost {
            (rng.gen_range(20..35), rng.gen_range(0.02..0.04))
        } else {
            // Accent on beat 1 and beat 5 (strong beats)
            let accent = if beat_in_bar == 0 || beat_in_bar == 4 {
                0.03
            } else {
                0.0
            };
            (base_dur, rng.gen_range(0.05..0.09) + accent)
        };

        // Occasional octave jump for emphasis on strong beats
        let freq = if beat_in_bar == 0 && rng.gen_range(0..5) == 0 {
            freq * 2.0  // pop up an octave on downbeat
        } else if is_ghost {
            freq * 2.0  // ghost notes an octave up (like a tap)
        } else {
            freq
        };

        self.play(
            Osc::sine(freq)
                .take_duration(Duration::from_millis(dur))
                .amplify(vol as f32)
                .fade_out(Duration::from_millis(dur))
        );
    }

    pub fn hit(&self, scale: &[f32]) {
        // Punch — low minor triad, random duration and waveform
        let mut rng = ::rand::thread_rng();
        let freqs: Vec<f32> = pick_minor_triad(scale, &mut rng).iter().map(|f| f * 0.25).collect();
        let dur = rng.gen_range(120..200);
        let wf = if rng.gen_bool(0.5) { Waveform::Square } else { Waveform::Saw };
        self.play(
            Chord::new(&freqs, wf)
                .take_duration(Duration::from_millis(dur))
                .amplify(rng.gen_range(0.12..0.18))
                .fade_out(Duration::from_millis(dur))
        );
    }

    pub fn crit(&self, scale: &[f32]) {
        // Hard slam — random between single big chord or quick double-hit
        let mut rng = ::rand::thread_rng();
        let freqs: Vec<f32> = pick_minor_triad(scale, &mut rng).iter().map(|f| f * 0.25).collect();
        if rng.gen_bool(0.4) {
            // Double-hit variant
            let d1 = rng.gen_range(80..130);
            let d2 = rng.gen_range(160..280);
            self.play(
                Chord::saw(&freqs)
                    .take_duration(Duration::from_millis(d1))
                    .amplify(0.20)
                    .then(silence(Duration::from_millis(rng.gen_range(20..50))))
                    .then(Chord::saw(&freqs)
                        .take_duration(Duration::from_millis(d2))
                        .amplify(0.25)
                        .fade_out(Duration::from_millis(d2)))
            );
        } else {
            let dur = rng.gen_range(180..300);
            self.play(
                Chord::saw(&freqs)
                    .take_duration(Duration::from_millis(dur))
                    .amplify(0.25)
                    .fade_out(Duration::from_millis(dur))
            );
        }
    }

    pub fn player_hurt(&self, scale: &[f32]) {
        // Descending dyad sweep — random sweep speed and hold
        let mut rng = ::rand::thread_rng();
        let hi = pick_dyad_high(scale, &mut rng);
        let lo = pick_dyad_low(scale, &mut rng);
        let hi_avg = hi.iter().sum::<f32>() / hi.len() as f32;
        let lo_avg = lo.iter().sum::<f32>() / lo.len() as f32;
        let sweep_dur = rng.gen_range(100..200);
        let hold_dur = rng.gen_range(80..180);
        let wf = if rng.gen_bool(0.6) { Waveform::Square } else { Waveform::Saw };
        self.play(
            Sweep::new(hi_avg, lo_avg, Duration::from_millis(sweep_dur), wf)
                .amplify(0.10)
                .fade_out(Duration::from_millis(sweep_dur))
                .then(
                    Chord::new(&lo, wf)
                        .take_duration(Duration::from_millis(hold_dur))
                        .amplify(0.12)
                        .fade_out(Duration::from_millis(hold_dur))
                )
        );
    }

    pub fn miss(&self, scale: &[f32]) {
        // Quick high dyad, very quiet — random pitch range and duration
        let mut rng = ::rand::thread_rng();
        let freqs = pick_dyad_high(scale, &mut rng);
        let dur = rng.gen_range(40..100);
        self.play(
            Chord::sine(&freqs)
                .take_duration(Duration::from_millis(dur))
                .amplify(rng.gen_range(0.02..0.05))
                .fade_out(Duration::from_millis(dur))
        );
    }

    pub fn kill(&self, scale: &[f32]) {
        // Rising sweep into a triumphant chord — random sweep/hold and optional grace note
        let mut rng = ::rand::thread_rng();
        let lo = pick_low(scale, &mut rng);
        let hi_chord = pick_triad(scale, &mut rng);
        let hi_avg = hi_chord.iter().sum::<f32>() / hi_chord.len() as f32;
        let sweep_dur = rng.gen_range(60..160);
        let hold_dur = rng.gen_range(200..450);

        if rng.gen_bool(0.3) {
            // Grace note variant: quick single note before the sweep
            let grace = pick_high(scale, &mut rng);
            let grace_dur = rng.gen_range(30..60);
            self.play(
                Osc::sine(grace).take_duration(Duration::from_millis(grace_dur)).amplify(0.08)
                    .then(Sweep::new(lo, hi_avg, Duration::from_millis(sweep_dur), Waveform::Sine)
                        .amplify(0.10))
                    .then(Chord::sine(&hi_chord)
                        .take_duration(Duration::from_millis(hold_dur))
                        .amplify(0.12)
                        .fade_out(Duration::from_millis(hold_dur)))
            );
        } else {
            self.play(
                Sweep::new(lo, hi_avg, Duration::from_millis(sweep_dur), Waveform::Sine)
                    .amplify(0.10)
                    .then(Chord::sine(&hi_chord)
                        .take_duration(Duration::from_millis(hold_dur))
                        .amplify(0.12)
                        .fade_out(Duration::from_millis(hold_dur)))
            );
        }
    }

    pub fn death(&self, scale: &[f32]) {
        // Funeral march — minor triads with vibrato warble
        // Chopin-inspired dotted rhythm: DUM da-da DUM, DUM da-da DUM
        let mut rng = ::rand::thread_rng();
        let npo = notes_per_octave(scale);

        // Random tempo (quarter note in ms)
        let quarter = rng.gen_range(350..550) as u64;
        let dotted_quarter = quarter * 3 / 2;
        let eighth = quarter / 2;
        let sixteenth = quarter / 4;

        let num_phrases = rng.gen_range(2..5);

        // Random starting position
        let max_start = scale.len().saturating_sub(npo + 2);
        let phrase_start = if max_start > 0 { rng.gen_range(0..max_start) } else { 0 };

        let wf = match rng.gen_range(0..4) {
            0 => Waveform::Sine,
            1 => Waveform::Square,
            _ => Waveform::Saw,
        };

        // Vibrato parameters — randomized per performance
        let vib_rate = rng.gen_range(3.5..6.5);    // LFO speed in Hz
        let vib_depth = rng.gen_range(0.008..0.025); // ±0.8% to ±2.5% pitch

        let mut song: Box<dyn Source<Item = f32> + Send> = Box::new(
            silence(Duration::from_millis(1))
        );

        for phrase in 0..num_phrases {
            let base = phrase_start + (npo.saturating_sub(phrase * 2)).min(scale.len().saturating_sub(5));

            // Force minor triad: root, minor third (3 semitones), perfect fifth (7 semitones)
            let root = scale[base.min(scale.len() - 1)] * 0.5;
            let minor_third = root * 2.0_f32.powf(3.0 / 12.0);  // always minor 3rd
            let fifth = root * 2.0_f32.powf(7.0 / 12.0);        // perfect 5th

            // Neighbor: half step below root (chromatic) for maximum darkness
            let neighbor = root * 2.0_f32.powf(-1.0 / 12.0);

            // Resolving chord — one whole step down, also minor
            let r2 = root * 2.0_f32.powf(-2.0 / 12.0);
            let t2 = r2 * 2.0_f32.powf(3.0 / 12.0);
            let f2 = r2 * 2.0_f32.powf(7.0 / 12.0);

            let vol = (0.14 - phrase as f32 * 0.015).max(0.06);
            let is_last = phrase == num_phrases - 1;

            let has_grace = rng.gen_bool(0.6);
            let has_pickup = rng.gen_bool(0.7);
            // Rubato per phrase
            let rubato: f64 = rng.gen_range(0.85..1.15);
            let q = (quarter as f64 * rubato) as u64;
            let dq = (dotted_quarter as f64 * rubato) as u64;
            let e = (eighth as f64 * rubato) as u64;
            let s = (sixteenth as f64 * rubato) as u64;

            // Vibrato deepens as the song progresses (dying warble)
            let phrase_vib = vib_depth * (1.0 + phrase as f32 * 0.3);

            // Beat 1: DUM — heavy dotted quarter, minor triad with vibrato
            if has_grace {
                song = Box::new(song
                    .then(Osc::new(neighbor, wf).with_vibrato(vib_rate, phrase_vib)
                        .take_duration(Duration::from_millis(s))
                        .amplify(vol * 0.5)));
            }
            song = Box::new(song
                .then(Chord::new(&[root, minor_third, fifth], wf)
                    .with_vibrato(vib_rate, phrase_vib)
                    .take_duration(Duration::from_millis(dq))
                    .amplify(vol)
                    .fade_out(Duration::from_millis(dq))));

            // Beats 2-3: da-da — two quick eighths
            if has_pickup {
                let pickup_vol = vol * 0.6;
                if rng.gen_bool(0.5) {
                    // Single minor third note
                    song = Box::new(song
                        .then(Osc::new(minor_third, wf).with_vibrato(vib_rate, phrase_vib)
                            .take_duration(Duration::from_millis(e))
                            .amplify(pickup_vol)));
                } else {
                    // Thin minor dyad
                    song = Box::new(song
                        .then(Chord::new(&[root, minor_third], wf)
                            .with_vibrato(vib_rate, phrase_vib)
                            .take_duration(Duration::from_millis(e))
                            .amplify(pickup_vol)));
                }
                song = Box::new(song
                    .then(Osc::new(neighbor, wf).with_vibrato(vib_rate, phrase_vib)
                        .take_duration(Duration::from_millis(e))
                        .amplify(pickup_vol)));
            } else {
                song = Box::new(song
                    .then(silence(Duration::from_millis(e * 2))));
            }

            // Beat 4: DUM — resolving minor chord with deeper vibrato
            let resolve_dur = if is_last {
                rng.gen_range(1000..1800)
            } else {
                q + rng.gen_range(0..e)
            };
            let resolve_vib = if is_last { phrase_vib * 1.5 } else { phrase_vib };
            song = Box::new(song
                .then(Chord::new(&[r2, t2, f2], wf)
                    .with_vibrato(vib_rate * 0.8, resolve_vib)
                    .take_duration(Duration::from_millis(resolve_dur))
                    .amplify(if is_last { vol + 0.02 } else { vol * 0.9 })
                    .fade_out(Duration::from_millis(resolve_dur))));

            if !is_last {
                let gap = rng.gen_range(80..200);
                song = Box::new(song.then(silence(Duration::from_millis(gap))));
            }
        }

        self.play(song);
    }

    pub fn victory(&self, scale: &[f32]) {
        // Early Beatles rock — jangly rhythm guitar, uses the level's actual scale/mode
        // Dark modes get minor feel, bright modes get major feel
        let mut rng = ::rand::thread_rng();
        let npo = notes_per_octave(scale);

        // Rock tempo — 140-180 BPM (varies per performance)
        let quarter = rng.gen_range(160..230) as u64;
        let eighth = quarter / 2;

        // Use the actual scale for chords — triads built from scale degrees
        // This makes dark modes (phrygian, aeolian) sound darker,
        // bright modes (ionian, lydian) sound brighter
        let triad_at = |deg: usize| -> Vec<f32> {
            let r = scale[deg % scale.len()];
            let t = scale[(deg + 2) % scale.len()];
            let f = scale[(deg + 4) % scale.len()];
            // Ensure third and fifth are above root
            let t = if t < r { t * 2.0 } else { t };
            let f = if f < t { f * 2.0 } else { f };
            vec![r, t, f]
        };

        // Random rhythm style
        let style = rng.gen_range(0..4);
        let guitar = match style {
            0 => Waveform::Square,  // jangly Rickenbacker
            1 => Waveform::Saw,     // rawer, more aggressive
            _ => Waveform::Square,
        };

        // Build a progression using actual scale degrees
        // Varies based on random choice — not always I-IV-V
        let prog_patterns: [[usize; 4]; 5] = [
            [0, 3, 4, 0],   // I-IV-V-I (classic)
            [0, 5, 3, 4],   // I-vi-IV-V (doo-wop)
            [0, 2, 4, 0],   // I-iii-V-I (open)
            [0, 4, 3, 4],   // I-V-IV-V (rock)
            [0, 3, 0, 4],   // I-IV-I-V (simple)
        ];
        let pattern = &prog_patterns[rng.gen_range(0..prog_patterns.len())];

        // 6-10 bars
        let num_bars = rng.gen_range(6..11);

        let mut song: Box<dyn Source<Item = f32> + Send> = Box::new(
            silence(Duration::from_millis(1))
        );

        for bar in 0..num_bars {
            let degree = pattern[bar % pattern.len()] % npo;
            let chord = triad_at(degree);
            let is_last = bar == num_bars - 1;

            match style {
                0 => {
                    // Driving eighths — chunk chunk chunk chunk
                    for beat in 0..4 {
                        let dur = eighth - rng.gen_range(8..18);
                        let gap = eighth - dur;
                        let vol = if beat % 2 == 0 { 0.10 } else { 0.07 };
                        song = Box::new(song
                            .then(Chord::new(&chord, guitar)
                                .take_duration(Duration::from_millis(dur))
                                .amplify(vol)
                                .fade_out(Duration::from_millis(dur)))
                            .then(silence(Duration::from_millis(gap))));
                    }
                }
                1 => {
                    // Syncopated — long-short-long
                    let long = quarter + eighth;
                    let short = eighth;
                    song = Box::new(song
                        .then(Chord::new(&chord, guitar)
                            .take_duration(Duration::from_millis(long))
                            .amplify(0.10)
                            .fade_out(Duration::from_millis(long)))
                        .then(Chord::new(&chord, guitar)
                            .take_duration(Duration::from_millis(short))
                            .amplify(0.07)
                            .fade_out(Duration::from_millis(short))));
                }
                2 => {
                    // Staccato stabs with rests — punchy
                    for _ in 0..2 {
                        let stab = rng.gen_range(40..80);
                        let rest = quarter - stab;
                        song = Box::new(song
                            .then(Chord::new(&chord, guitar)
                                .take_duration(Duration::from_millis(stab))
                                .amplify(0.12))
                            .then(silence(Duration::from_millis(rest))));
                    }
                }
                _ => {
                    // Arpeggiated — pick individual notes of the chord
                    for &note in &chord {
                        let dur = rng.gen_range(60..100);
                        let gap = rng.gen_range(10..30);
                        song = Box::new(song
                            .then(Osc::new(note, guitar)
                                .take_duration(Duration::from_millis(dur))
                                .amplify(0.08)
                                .fade_out(Duration::from_millis(dur)))
                            .then(silence(Duration::from_millis(gap))));
                    }
                    // Fill remaining time
                    song = Box::new(song.then(silence(Duration::from_millis(
                        (quarter * 2).saturating_sub(chord.len() as u64 * 100)
                    ))));
                }
            }

            // Fill every 4th bar — ascending scale run
            if bar % 4 == 3 && !is_last {
                let fill_len = rng.gen_range(3..6);
                let fill_dur = rng.gen_range(30..60);
                for i in 0..fill_len {
                    let n = scale[(degree + i) % scale.len()];
                    let n = if i > 0 && n < scale[degree % scale.len()] { n * 2.0 } else { n };
                    song = Box::new(song
                        .then(Osc::new(n, guitar)
                            .take_duration(Duration::from_millis(fill_dur))
                            .amplify(0.06 + i as f32 * 0.005)
                            .fade_out(Duration::from_millis(fill_dur))));
                }
            }
        }

        // Finish: variable number of stabs then resolve
        let resolve_deg = if rng.gen_bool(0.5) { 3 } else { 4 };
        let resolve_chord = triad_at(resolve_deg % npo);
        let hit_chord = triad_at(0);
        let gap = rng.gen_range(50..90);
        let hold = rng.gen_range(500..900);
        song = Box::new(song.then(silence(Duration::from_millis(gap))));

        let ending = rng.gen_range(0..4);
        match ending {
            0 => {
                // Single big resolve
                song = Box::new(song
                    .then(Chord::new(&resolve_chord, Waveform::Saw)
                        .take_duration(Duration::from_millis(hold)).amplify(0.16)
                        .fade_out(Duration::from_millis(hold))));
            }
            1 => {
                // Two hits: stab → resolve
                let hit = rng.gen_range(70..120);
                song = Box::new(song
                    .then(Chord::new(&hit_chord, guitar)
                        .take_duration(Duration::from_millis(hit)).amplify(0.13))
                    .then(silence(Duration::from_millis(gap)))
                    .then(Chord::new(&resolve_chord, Waveform::Saw)
                        .take_duration(Duration::from_millis(hold)).amplify(0.16)
                        .fade_out(Duration::from_millis(hold))));
            }
            2 => {
                // Three hits: stab stab → resolve
                let hit = rng.gen_range(70..120);
                song = Box::new(song
                    .then(Chord::new(&hit_chord, guitar)
                        .take_duration(Duration::from_millis(hit)).amplify(0.13))
                    .then(silence(Duration::from_millis(gap)))
                    .then(Chord::new(&hit_chord, guitar)
                        .take_duration(Duration::from_millis(hit)).amplify(0.14))
                    .then(silence(Duration::from_millis(gap)))
                    .then(Chord::new(&resolve_chord, Waveform::Saw)
                        .take_duration(Duration::from_millis(hold)).amplify(0.16)
                        .fade_out(Duration::from_millis(hold))));
            }
            _ => {
                // Ascending run into resolve
                let run_len = rng.gen_range(3..6);
                let run_dur = rng.gen_range(40..70);
                for i in 0..run_len {
                    let n = scale[i % scale.len()];
                    let n = if i > 0 && n < scale[0] { n * 2.0 } else { n };
                    song = Box::new(song
                        .then(Osc::new(n, guitar)
                            .take_duration(Duration::from_millis(run_dur))
                            .amplify(0.07 + i as f32 * 0.01)
                            .fade_out(Duration::from_millis(run_dur))));
                }
                song = Box::new(song
                    .then(silence(Duration::from_millis(gap)))
                    .then(Chord::new(&resolve_chord, Waveform::Saw)
                        .take_duration(Duration::from_millis(hold)).amplify(0.16)
                        .fade_out(Duration::from_millis(hold))));
            }
        }

        self.play(song);
    }

    pub fn pickup_gold(&self, scale: &[f32]) {
        // High sparkle — random between single dyad and double-tap
        let mut rng = ::rand::thread_rng();
        let freqs: Vec<f32> = pick_dyad_high(scale, &mut rng).iter().map(|f| f * 2.0).collect();
        if rng.gen_bool(0.35) {
            // Double-tap sparkle
            let d1 = rng.gen_range(40..80);
            let d2 = rng.gen_range(60..140);
            self.play(
                Chord::sine(&freqs)
                    .take_duration(Duration::from_millis(d1))
                    .amplify(0.08)
                    .then(silence(Duration::from_millis(rng.gen_range(15..35))))
                    .then(Chord::sine(&freqs)
                        .take_duration(Duration::from_millis(d2))
                        .amplify(0.10)
                        .fade_out(Duration::from_millis(d2)))
            );
        } else {
            let dur = rng.gen_range(80..160);
            self.play(
                Chord::sine(&freqs)
                    .take_duration(Duration::from_millis(dur))
                    .amplify(0.10)
                    .fade_out(Duration::from_millis(dur))
            );
        }
    }

    pub fn pickup_potion(&self, scale: &[f32]) {
        // Rising sweep into a dyad — random sweep speed and optional bubble effect
        let mut rng = ::rand::thread_rng();
        let lo = pick_low(scale, &mut rng);
        let hi_chord = pick_dyad(scale, &mut rng);
        let hi_avg = hi_chord.iter().sum::<f32>() / hi_chord.len() as f32;
        let sweep_dur = rng.gen_range(50..120);
        let hold_dur = rng.gen_range(70..150);
        if rng.gen_bool(0.3) {
            // Bubble: two quick sweeps then chord
            let s1 = rng.gen_range(30..50);
            let mid = (lo + hi_avg) / 2.0;
            self.play(
                Sweep::new(lo, mid, Duration::from_millis(s1), Waveform::Sine).amplify(0.06)
                    .then(Sweep::new(mid * 0.95, hi_avg, Duration::from_millis(sweep_dur), Waveform::Sine).amplify(0.08))
                    .then(Chord::sine(&hi_chord)
                        .take_duration(Duration::from_millis(hold_dur))
                        .amplify(0.10)
                        .fade_out(Duration::from_millis(hold_dur)))
            );
        } else {
            self.play(
                Sweep::new(lo, hi_avg, Duration::from_millis(sweep_dur), Waveform::Sine)
                    .amplify(0.08)
                    .then(Chord::sine(&hi_chord)
                        .take_duration(Duration::from_millis(hold_dur))
                        .amplify(0.10)
                        .fade_out(Duration::from_millis(hold_dur)))
            );
        }
    }

    pub fn pickup_weapon(&self, scale: &[f32]) {
        // Ascending metallic fanfare — random chord count and rhythm
        let mut rng = ::rand::thread_rng();
        let num_chords = rng.gen_range(2..5);
        let notes = pick_ascending(scale, num_chords * 2 + 1, &mut rng);

        let mut song: Box<dyn Source<Item = f32> + Send> = Box::new(
            silence(Duration::from_millis(1))
        );
        for i in 0..num_chords {
            let n0 = notes[i.min(notes.len() - 1)];
            let n1 = notes[(i + 2).min(notes.len() - 1)];
            let is_last = i == num_chords - 1;
            let dur = if is_last {
                rng.gen_range(280..500)
            } else {
                rng.gen_range(70..160)
            };
            let gap = if is_last { 0 } else { rng.gen_range(15..60) };
            let vol = if is_last { 0.12 } else { 0.10 };
            let chord = Chord::saw(&[n0, n1]);
            if is_last {
                song = Box::new(song
                    .then(chord.take_duration(Duration::from_millis(dur)).amplify(vol)
                        .fade_out(Duration::from_millis(dur))));
            } else {
                song = Box::new(song
                    .then(chord.take_duration(Duration::from_millis(dur)).amplify(vol))
                    .then(silence(Duration::from_millis(gap))));
            }
        }
        self.play(song);
    }

    pub fn pickup_armor(&self, scale: &[f32]) {
        // Ascending metallic fanfare — one octave lower, random rhythm
        let mut rng = ::rand::thread_rng();
        let num_chords = rng.gen_range(2..5);
        let notes = pick_ascending(scale, num_chords * 2 + 1, &mut rng);

        let mut song: Box<dyn Source<Item = f32> + Send> = Box::new(
            silence(Duration::from_millis(1))
        );
        for i in 0..num_chords {
            let n0 = notes[i.min(notes.len() - 1)] * 0.5;
            let n1 = notes[(i + 2).min(notes.len() - 1)] * 0.5;
            let is_last = i == num_chords - 1;
            let dur = if is_last {
                rng.gen_range(280..500)
            } else {
                rng.gen_range(70..160)
            };
            let gap = if is_last { 0 } else { rng.gen_range(15..60) };
            let vol = if is_last { 0.12 } else { 0.10 };
            let chord = Chord::saw(&[n0, n1]);
            if is_last {
                song = Box::new(song
                    .then(chord.take_duration(Duration::from_millis(dur)).amplify(vol)
                        .fade_out(Duration::from_millis(dur))));
            } else {
                song = Box::new(song
                    .then(chord.take_duration(Duration::from_millis(dur)).amplify(vol))
                    .then(silence(Duration::from_millis(gap))));
            }
        }
        self.play(song);
    }

    pub fn pickup_key(&self, scale: &[f32]) {
        // Squarepusher-inspired: frenetic drill'n'bass glitch — rapid-fire bass stabs
        // with irregular timing, pitch bends, and a final low thud
        let mut rng = ::rand::thread_rng();
        let sq = Waveform::Square;

        let mut song: Box<dyn Source<Item = f32> + Send> = Box::new(
            silence(Duration::from_millis(1))
        );

        // Rapid glitch burst: 8-12 very short bass stabs at irregular intervals
        let num_stabs = rng.gen_range(8..13);
        for _ in 0..num_stabs {
            let idx = rng.gen_range(0..scale.len());
            let freq = scale[idx] * if rng.gen_bool(0.5) { 0.5 } else { 1.0 };
            let dur = rng.gen_range(20..60);
            let gap_ms = rng.gen_range(10..40);
            let note = Osc::new(freq, sq)
                .take_duration(Duration::from_millis(dur))
                .amplify(0.09)
                .fade_out(Duration::from_millis(dur.min(30)));
            song = Box::new(song.then(note).then(silence(Duration::from_millis(gap_ms))));
        }

        // Brief pause
        song = Box::new(song.then(silence(Duration::from_millis(rng.gen_range(30..60)))));

        // Final: descending bass slide (3 rapid notes going down)
        for i in 0..3 {
            let idx = (scale.len() / 2).saturating_sub(i);
            let freq = scale[idx.min(scale.len() - 1)] * 0.25;
            let dur = 40 + i as u64 * 20;
            let note = Osc::new(freq, sq)
                .take_duration(Duration::from_millis(dur))
                .amplify(0.12)
                .fade_out(Duration::from_millis(dur));
            song = Box::new(song.then(note));
        }

        self.play(song);
    }

    pub fn door_unlock(&self, scale: &[f32]) {
        // Heavy stone-door sound: deep rumble → ascending chime cascade
        if scale.is_empty() { return; }
        let mut rng = ::rand::thread_rng();
        let mut song: Box<dyn Source<Item = f32> + Send> = Box::new(
            silence(Duration::from_millis(1))
        );
        // Deep rumble: low sine sweep
        let base = scale[0] * 0.25;
        for i in 0..6 {
            let freq = base + i as f32 * 3.0;
            let note = Osc::new(freq, Waveform::Sine)
                .take_duration(Duration::from_millis(60))
                .amplify(0.08);
            song = Box::new(song.then(note));
        }
        // Brief silence
        song = Box::new(song.then(silence(Duration::from_millis(80))));
        // Rising chime: 4 ascending notes from the scale
        let n = scale.len();
        for i in 0..4 {
            let idx = (n / 2 + i).min(n - 1);
            let freq = scale[idx];
            let note = Osc::new(freq, Waveform::Sine)
                .take_duration(Duration::from_millis(120))
                .amplify(0.06)
                .fade_out(Duration::from_millis(100));
            song = Box::new(song.then(note));
        }
        // Final chord
        if n >= 3 {
            let chord = vec![scale[0], scale[n / 3], scale[2 * n / 3]];
            song = Box::new(song.then(
                Chord::sine(&chord)
                    .take_duration(Duration::from_millis(300))
                    .amplify(0.05)
                    .fade_out(Duration::from_millis(250))
            ));
        }
        self.play(song);
    }

    pub fn level_up(&self, scale: &[f32]) {
        // Mozart-style celebratory piece — scalar runs, Alberti bass, trills, cadence
        // Structured as: opening flourish → melodic phrase → cadential finish
        let mut rng = ::rand::thread_rng();
        let npo = notes_per_octave(scale);

        // Allegro tempo — eighth note in ms
        let eighth = rng.gen_range(80..130) as u64;
        let sixteenth = eighth / 2;
        let quarter = eighth * 2;
        let half = quarter * 2;

        // Force major for that bright classical sound
        let make_major = |freq: f32| -> [f32; 3] {
            [freq, freq * 2.0_f32.powf(4.0 / 12.0), freq * 2.0_f32.powf(7.0 / 12.0)]
        };

        // Sine for the melody (clarinet/flute-like), saw for bass
        let melody_wf = Waveform::Sine;
        let bass_wf = Waveform::Saw;

        // Pick a random starting degree for variety
        let start_deg = rng.gen_range(0..npo.min(3));

        // Helper: get a scale note safely, wrapping octaves
        let note_at = |deg: usize| -> f32 {
            if scale.is_empty() { return 440.0; }
            scale[deg.min(scale.len() - 1)]
        };

        let mut song: Box<dyn Source<Item = f32> + Send> = Box::new(
            silence(Duration::from_millis(1))
        );

        // ═══════════════════════════════════════════
        // SECTION 1: Opening flourish (scalar run up)
        // ═══════════════════════════════════════════
        let flourish_style = rng.gen_range(0..3);
        match flourish_style {
            0 => {
                // Fast ascending scale run (like a Mozart piano sonata opening)
                let run_len = rng.gen_range(5..9);
                for i in 0..run_len {
                    let deg = start_deg + i;
                    let n = note_at(deg);
                    let dur = sixteenth + rng.gen_range(0..sixteenth / 3);
                    let vol = 0.06 + (i as f32 / run_len as f32) * 0.04;
                    song = Box::new(song
                        .then(Osc::new(n, melody_wf)
                            .take_duration(Duration::from_millis(dur))
                            .amplify(vol)));
                }
                // Land on a triad at the top
                let top = note_at(start_deg + rng.gen_range(5..9).min(scale.len().saturating_sub(1)));
                let chord = make_major(top);
                song = Box::new(song
                    .then(Chord::sine(&chord)
                        .take_duration(Duration::from_millis(quarter))
                        .amplify(0.11)
                        .fade_out(Duration::from_millis(quarter))));
            }
            1 => {
                // Rocket theme — broken triad ascending (very Mozart)
                // Do-Mi-Sol-Do' played as quick sixteenths
                let root = note_at(start_deg);
                let maj = make_major(root);
                let octave = note_at(start_deg + npo);
                let rocket = [maj[0], maj[1], maj[2], octave];
                for (i, &n) in rocket.iter().enumerate() {
                    let dur = sixteenth + rng.gen_range(0..sixteenth / 2);
                    let vol = 0.07 + i as f32 * 0.015;
                    song = Box::new(song
                        .then(Osc::new(n, melody_wf)
                            .take_duration(Duration::from_millis(dur))
                            .amplify(vol)));
                }
                song = Box::new(song
                    .then(silence(Duration::from_millis(rng.gen_range(30..80)))));
            }
            _ => {
                // Trill + step up — ornamental opening
                let trill_note = note_at(start_deg + npo);
                let upper = note_at(start_deg + npo + 1);
                let num_trills = rng.gen_range(3..6);
                for i in 0..num_trills {
                    let n = if i % 2 == 0 { trill_note } else { upper };
                    song = Box::new(song
                        .then(Osc::new(n, melody_wf)
                            .take_duration(Duration::from_millis(sixteenth))
                            .amplify(0.08)));
                }
                // Resolve down a step
                song = Box::new(song
                    .then(Osc::new(trill_note, melody_wf)
                        .take_duration(Duration::from_millis(eighth))
                        .amplify(0.09)
                        .fade_out(Duration::from_millis(eighth))));
                song = Box::new(song
                    .then(silence(Duration::from_millis(rng.gen_range(30..60)))));
            }
        }

        // ═══════════════════════════════════════════
        // SECTION 2: Melodic phrase with accompaniment
        // ═══════════════════════════════════════════
        let phrase_style = rng.gen_range(0..3);
        let num_beats = rng.gen_range(4..8);

        // Build a melody contour
        let mut melody_deg = start_deg + npo; // start in upper octave
        for beat in 0..num_beats {
            let mel = note_at(melody_deg);
            let bass_root = note_at(start_deg + (beat % npo));
            let bass_chord = make_major(bass_root * 0.5);

            let rubato: f64 = rng.gen_range(0.9..1.1);
            let q = (quarter as f64 * rubato) as u64;
            let e = (eighth as f64 * rubato) as u64;
            let s = (sixteenth as f64 * rubato) as u64;

            match phrase_style {
                0 => {
                    // Alberti bass pattern: low-high-mid-high under melody
                    // Bass plays broken chord in sixteenths while melody holds
                    let alberti = [bass_chord[0], bass_chord[2], bass_chord[1], bass_chord[2]];
                    for (i, &bn) in alberti.iter().enumerate() {
                        // Melody on beat 1 and 3, bass throughout
                        if i == 0 || i == 2 {
                            song = Box::new(song
                                .then(Osc::new(bn, bass_wf)
                                    .take_duration(Duration::from_millis(s))
                                    .amplify(0.04)));
                        } else {
                            song = Box::new(song
                                .then(Osc::new(bn, bass_wf)
                                    .take_duration(Duration::from_millis(s))
                                    .amplify(0.03)));
                        }
                    }
                    // Melody note over the top (layered separately after bass pattern)
                    // Since we can't truly layer, play melody as the "beat 1" sound
                    // Rewrite: interleave melody + bass
                }
                1 => {
                    // Simple melody + bass: bass on 1, melody on 1-and
                    song = Box::new(song
                        .then(Osc::new(bass_chord[0], bass_wf)
                            .take_duration(Duration::from_millis(e))
                            .amplify(0.05)
                            .fade_out(Duration::from_millis(e)))
                        .then(Osc::new(mel, melody_wf)
                            .take_duration(Duration::from_millis(e))
                            .amplify(0.09)
                            .fade_out(Duration::from_millis(e))));
                }
                _ => {
                    // Melody in thirds (very Mozart — parallel thirds)
                    let third_above = note_at(melody_deg + 2);
                    song = Box::new(song
                        .then(Chord::sine(&[mel, third_above])
                            .take_duration(Duration::from_millis(q))
                            .amplify(0.09)
                            .fade_out(Duration::from_millis(q))));
                }
            }

            // Move melody — mostly stepwise, occasional leap
            let step: i32 = match rng.gen_range(0..10) {
                0..=3 => -1,   // step down (classical melodies often arc down)
                4..=5 => 1,    // step up
                6 => -2,       // leap down a third
                7 => 2,        // leap up a third
                8 => -3,       // leap down a fourth
                _ => 0,        // repeat
            };
            melody_deg = ((melody_deg as i32 + step).max(start_deg as i32)) as usize;
            melody_deg = melody_deg.min(scale.len() - 1);
        }

        // ═══════════════════════════════════════════
        // SECTION 3: Cadential finish
        // ═══════════════════════════════════════════
        let cadence_style = rng.gen_range(0..4);
        let tonic = note_at(start_deg);
        let tonic_chord = make_major(tonic);
        // V chord (dominant) — 5th scale degree, or approximate
        let dominant_root = note_at(start_deg + 4); // 5th degree in 7-note scale
        let dominant_chord = make_major(dominant_root);
        // IV chord (subdominant)
        let subdominant_root = note_at(start_deg + 3);
        let subdominant_chord = make_major(subdominant_root);

        song = Box::new(song.then(silence(Duration::from_millis(rng.gen_range(60..150)))));

        match cadence_style {
            0 => {
                // Perfect cadence: V → I with a trill on the leading tone
                // Trill first
                let leading = note_at(start_deg + npo - 1);
                let tonic_upper = note_at(start_deg + npo);
                let trills = rng.gen_range(2..5);
                for i in 0..trills {
                    let n = if i % 2 == 0 { leading } else { tonic_upper };
                    song = Box::new(song
                        .then(Osc::new(n, melody_wf)
                            .take_duration(Duration::from_millis(sixteenth))
                            .amplify(0.08)));
                }
                // V chord
                song = Box::new(song
                    .then(Chord::new(&dominant_chord, bass_wf)
                        .take_duration(Duration::from_millis(quarter))
                        .amplify(0.11)
                        .fade_out(Duration::from_millis(quarter))));
                // I chord — big and held
                let hold = rng.gen_range(600..1200);
                song = Box::new(song
                    .then(Chord::new(&[tonic * 0.5, tonic_chord[0], tonic_chord[1], tonic_chord[2]], melody_wf)
                        .take_duration(Duration::from_millis(hold))
                        .amplify(0.14)
                        .fade_out(Duration::from_millis(hold))));
            }
            1 => {
                // IV → V → I (classical full cadence)
                song = Box::new(song
                    .then(Chord::sine(&subdominant_chord)
                        .take_duration(Duration::from_millis(quarter))
                        .amplify(0.10)
                        .fade_out(Duration::from_millis(quarter)))
                    .then(Chord::new(&dominant_chord, bass_wf)
                        .take_duration(Duration::from_millis(quarter))
                        .amplify(0.11)
                        .fade_out(Duration::from_millis(quarter))));
                // Dramatic pause
                song = Box::new(song
                    .then(silence(Duration::from_millis(rng.gen_range(80..180)))));
                let hold = rng.gen_range(800..1400);
                song = Box::new(song
                    .then(Chord::new(&[tonic * 0.5, tonic_chord[0], tonic_chord[1], tonic_chord[2]], melody_wf)
                        .take_duration(Duration::from_millis(hold))
                        .amplify(0.15)
                        .fade_out(Duration::from_millis(hold))));
            }
            2 => {
                // Ascending scale run into final chord (like a Mozart concerto ending)
                let run_len = rng.gen_range(4..8);
                for i in 0..run_len {
                    let n = note_at(start_deg + i);
                    let dur = sixteenth - (i as u64 * sixteenth / (run_len as u64 * 3)).min(sixteenth / 2);
                    song = Box::new(song
                        .then(Osc::new(n, melody_wf)
                            .take_duration(Duration::from_millis(dur.max(20)))
                            .amplify(0.07 + i as f32 * 0.008)));
                }
                // Three staccato chords: I I I!
                let stac = rng.gen_range(80..140);
                let gap = rng.gen_range(40..80);
                song = Box::new(song
                    .then(Chord::sine(&tonic_chord)
                        .take_duration(Duration::from_millis(stac)).amplify(0.11))
                    .then(silence(Duration::from_millis(gap)))
                    .then(Chord::sine(&tonic_chord)
                        .take_duration(Duration::from_millis(stac)).amplify(0.12))
                    .then(silence(Duration::from_millis(gap))));
                let hold = rng.gen_range(700..1200);
                song = Box::new(song
                    .then(Chord::new(&[tonic * 0.5, tonic_chord[0], tonic_chord[1], tonic_chord[2]], melody_wf)
                        .take_duration(Duration::from_millis(hold))
                        .amplify(0.15)
                        .fade_out(Duration::from_millis(hold))));
            }
            _ => {
                // Deceptive cadence → real cadence (surprise! then resolution)
                // V → vi (surprise)
                let deceptive_root = note_at(start_deg + 5); // vi degree
                let deceptive_chord = make_major(deceptive_root);
                song = Box::new(song
                    .then(Chord::new(&dominant_chord, bass_wf)
                        .take_duration(Duration::from_millis(quarter))
                        .amplify(0.10)
                        .fade_out(Duration::from_millis(quarter)))
                    .then(Chord::sine(&deceptive_chord)
                        .take_duration(Duration::from_millis(half))
                        .amplify(0.09)
                        .fade_out(Duration::from_millis(half))));
                // Pause for dramatic effect
                song = Box::new(song
                    .then(silence(Duration::from_millis(rng.gen_range(120..250)))));
                // Now the real cadence: V → I
                song = Box::new(song
                    .then(Chord::new(&dominant_chord, bass_wf)
                        .take_duration(Duration::from_millis(quarter))
                        .amplify(0.11)
                        .fade_out(Duration::from_millis(quarter))));
                let hold = rng.gen_range(900..1500);
                song = Box::new(song
                    .then(Chord::new(&[tonic * 0.5, tonic_chord[0], tonic_chord[1], tonic_chord[2]], melody_wf)
                        .take_duration(Duration::from_millis(hold))
                        .amplify(0.16)
                        .fade_out(Duration::from_millis(hold))));
            }
        }

        self.play(song);
    }

    pub fn trap(&self, scale: &[f32]) {
        // Dissonant low chord stab — random detune amount and rhythm
        let mut rng = ::rand::thread_rng();
        let freqs: Vec<f32> = pick_triad(scale, &mut rng).iter().map(|f| f * 0.25).collect();
        let detune = rng.gen_range(0.85..0.95);
        let detuned: Vec<f32> = freqs.iter().map(|f| f * detune).collect();
        let stab_dur = rng.gen_range(30..90);
        let hold_dur = rng.gen_range(200..400);
        let wf = if rng.gen_bool(0.7) { Waveform::Square } else { Waveform::Saw };
        self.play(
            Chord::new(&freqs, wf)
                .take_duration(Duration::from_millis(stab_dur))
                .amplify(0.15)
                .then(
                    Chord::new(&detuned, wf)
                        .take_duration(Duration::from_millis(hold_dur))
                        .amplify(0.12)
                        .fade_out(Duration::from_millis(hold_dur))
                )
        );
    }

    pub fn bomb(&self, scale: &[f32]) {
        // Explosion: low thud + noise burst fading out
        let mut rng = ::rand::thread_rng();
        let root = pick_low(scale, &mut rng) * 0.25;

        // Pre-render noise burst to a buffer (Noise uses ThreadRng which isn't Send)
        let boom_dur = rng.gen_range(300..500);
        let boom_samples = (44100.0 * boom_dur as f32 / 1000.0) as usize;
        let mut noise_buf: Vec<f32> = Vec::with_capacity(boom_samples);
        for i in 0..boom_samples {
            let t = i as f32 / boom_samples as f32;
            let fade = 1.0 - t; // linear fade out
            noise_buf.push(rng.gen_range(-1.0_f32..1.0) * 0.18 * fade);
        }
        let boom = rodio::buffer::SamplesBuffer::new(1, 44100, noise_buf);

        // Initial impact: very short low thud → noise explosion
        let thud_dur = rng.gen_range(40..70);
        let thud = Osc::new(root, Waveform::Square)
            .take_duration(Duration::from_millis(thud_dur))
            .amplify(0.25);
        let song = thud.then(boom);

        // Low rumble underneath
        let rumble_dur = boom_dur + 100;
        let rumble = Osc::new(root * 0.5, Waveform::Saw)
            .take_duration(Duration::from_millis(rumble_dur))
            .amplify(0.10)
            .fade_out(Duration::from_millis(rumble_dur));

        self.play(song);
        self.play(rumble);
    }

    pub fn boss_kill(&self, scale: &[f32]) {
        // Shriek: high dissonant sweep descending into silence
        let mut rng = ::rand::thread_rng();
        let hi = pick_high(scale, &mut rng) * 4.0;
        let lo = pick_low(scale, &mut rng);
        let shriek_dur = rng.gen_range(400..700);

        // Main shriek: high saw sweep down
        let shriek = Sweep::new(hi, lo, Duration::from_millis(shriek_dur), Waveform::Saw)
            .amplify(0.20)
            .fade_out(Duration::from_millis(shriek_dur));

        // Dissonant overtone slightly detuned
        let shriek2 = Sweep::new(hi * 1.03, lo * 0.97, Duration::from_millis(shriek_dur), Waveform::Square)
            .amplify(0.10)
            .fade_out(Duration::from_millis(shriek_dur));

        self.play(shriek);
        self.play(shriek2);
    }

    /// Start a looping low drone for boss proximity. Call once when a level begins.
    pub fn start_boss_drone(&self, scale: &[f32]) {
        self.stop_boss_drone();
        let root = if scale.is_empty() { 55.0 } else { scale[0] * 0.25 };
        // Use the second scale degree for dissonance — dark modes (phrygian, locrian)
        // naturally have a minor 2nd here, bright modes have a major 2nd
        let second = if scale.len() >= 2 { scale[1] * 0.25 } else { root * 1.06 };
        let source = Chord::new(&[root, second, root * 1.005], Waveform::Sine);
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

    pub fn cheat_fanfare(&self) {
        // Devo-style new wave fanfare — staccato square wave, mechanical, punchy
        let beat = 120u64; // ms per note — fast and robotic
        let gap = 30u64;
        let sq = Waveform::Square;

        // "Whip It" inspired riff: rhythmic staccato ascending then big finish
        let notes: &[(f32, u64)] = &[
            // Staccato intro — E4 E4 E4 G4 (robotic stabs)
            (329.6, beat), (329.6, beat), (329.6, beat), (392.0, beat),
            // Gap
            (0.0, gap * 2),
            // Ascending chromatic run — A4 Bb4 B4 C5
            (440.0, beat), (466.2, beat), (493.9, beat), (523.3, beat),
            // Gap
            (0.0, gap * 2),
            // Big finish — E5 stab, rest, E5-G5 power chord
            (659.3, beat * 2),
            (0.0, gap * 3),
            (659.3, beat * 3), // held final note
        ];

        let mut song: Box<dyn Source<Item = f32> + Send> = Box::new(
            silence(Duration::from_millis(1))
        );

        for &(freq, dur) in notes {
            if freq < 1.0 {
                song = Box::new(song.then(silence(Duration::from_millis(dur))));
            } else {
                let note_dur = Duration::from_millis(dur.saturating_sub(gap));
                let note = Osc::new(freq, sq)
                    .take_duration(note_dur)
                    .amplify(0.10)
                    .fade_out(Duration::from_millis(gap.min(dur)));
                let rest = silence(Duration::from_millis(gap));
                song = Box::new(song.then(note).then(rest));
            }
        }

        // Layer a low octave doubling for thickness
        let mut bass: Box<dyn Source<Item = f32> + Send> = Box::new(
            silence(Duration::from_millis(1))
        );
        for &(freq, dur) in notes {
            if freq < 1.0 {
                bass = Box::new(bass.then(silence(Duration::from_millis(dur))));
            } else {
                let note_dur = Duration::from_millis(dur.saturating_sub(gap));
                let note = Osc::new(freq * 0.5, sq)
                    .take_duration(note_dur)
                    .amplify(0.06)
                    .fade_out(Duration::from_millis(gap.min(dur)));
                let rest = silence(Duration::from_millis(gap));
                bass = Box::new(bass.then(note).then(rest));
            }
        }

        self.play(song);
        self.play(bass);
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
    fn saw(freqs: &[f32]) -> Self { Self::new(freqs, Waveform::Saw) }
    /// Add vibrato to all oscillators in the chord. Each voice gets a slightly
    /// different LFO phase so they don't all wobble in lockstep.
    fn with_vibrato(mut self, rate: f32, depth: f32) -> Self {
        for (i, osc) in self.oscs.iter_mut().enumerate() {
            osc.vib_rate = rate;
            osc.vib_depth = depth;
            osc.vib_phase = i as f32 * 0.3; // stagger LFO phases
        }
        self
    }
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
    // Vibrato LFO: sinusoidal pitch modulation
    vib_rate: f32,   // LFO speed in Hz (0 = off)
    vib_depth: f32,  // pitch deviation as a ratio (e.g. 0.02 = ±2% = ~35 cents)
    vib_phase: f32,
}

impl Osc {
    fn new(freq: f32, waveform: Waveform) -> Self {
        Self { freq, sample_rate: 44100, phase: 0.0, waveform,
               vib_rate: 0.0, vib_depth: 0.0, vib_phase: 0.0 }
    }
    fn sine(freq: f32) -> Self { Self::new(freq, Waveform::Sine) }
    fn with_vibrato(mut self, rate: f32, depth: f32) -> Self {
        self.vib_rate = rate;
        self.vib_depth = depth;
        self
    }
}

impl Iterator for Osc {
    type Item = f32;
    fn next(&mut self) -> Option<f32> {
        let val = match self.waveform {
            Waveform::Sine => (self.phase * std::f32::consts::TAU).sin(),
            Waveform::Square => if self.phase < 0.5 { 1.0 } else { -1.0 },
            Waveform::Saw => 2.0 * self.phase - 1.0,
        };
        // Apply vibrato: modulate instantaneous frequency
        let freq = if self.vib_rate > 0.0 {
            let lfo = (self.vib_phase * std::f32::consts::TAU).sin();
            self.vib_phase = (self.vib_phase + self.vib_rate / self.sample_rate as f32) % 1.0;
            self.freq * (1.0 + self.vib_depth * lfo)
        } else {
            self.freq
        };
        self.phase = (self.phase + freq / self.sample_rate as f32) % 1.0;
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

/// Simple multi-tap reverb using 4 comb filters at different prime-ratio delays.
/// Each tap feeds back into its own delay line, creating a diffuse decay.
struct Reverb<S> {
    source: S,
    sample_rate: u32,
    buffers: [Vec<f32>; 4],   // 4 delay lines at different lengths
    write_pos: [usize; 4],
    feedback: f32,
    wet: f32,
}

impl<S: Source<Item = f32>> Reverb<S> {
    fn new(source: S, delay_ms: f32, feedback: f32, wet: f32) -> Self {
        let sr = source.sample_rate();
        // 4 taps at prime-ratio multiples of the base delay to avoid metallic resonance
        let base_samples = (delay_ms * sr as f32 / 1000.0) as usize;
        let tap_lengths = [
            (base_samples as f32 * 1.0) as usize,
            (base_samples as f32 * 1.31) as usize,   // ~prime ratio
            (base_samples as f32 * 1.69) as usize,
            (base_samples as f32 * 2.13) as usize,
        ];
        let buffers = [
            vec![0.0f32; tap_lengths[0].max(1)],
            vec![0.0f32; tap_lengths[1].max(1)],
            vec![0.0f32; tap_lengths[2].max(1)],
            vec![0.0f32; tap_lengths[3].max(1)],
        ];
        Reverb {
            source, sample_rate: sr, buffers, write_pos: [0; 4],
            feedback: feedback.clamp(0.0, 0.85), wet: wet.clamp(0.0, 0.6),
        }
    }
}

impl<S: Source<Item = f32>> Iterator for Reverb<S> {
    type Item = f32;
    fn next(&mut self) -> Option<f32> {
        let dry = self.source.next()?;

        // Sum the delayed taps
        let mut wet_sum = 0.0f32;
        for (i, buf) in self.buffers.iter().enumerate() {
            let read_pos = self.write_pos[i];
            wet_sum += buf[read_pos];
        }
        wet_sum /= 4.0; // average the taps

        // Write input + feedback into each delay line
        for (i, buf) in self.buffers.iter_mut().enumerate() {
            let pos = self.write_pos[i];
            let delayed = buf[pos];
            buf[pos] = dry + delayed * self.feedback;
            self.write_pos[i] = (pos + 1) % buf.len();
        }

        Some(dry * (1.0 - self.wet) + wet_sum * self.wet)
    }
}

impl<S: Source<Item = f32>> Source for Reverb<S> {
    fn current_frame_len(&self) -> Option<usize> { self.source.current_frame_len() }
    fn channels(&self) -> u16 { self.source.channels() }
    fn sample_rate(&self) -> u32 { self.sample_rate }
    fn total_duration(&self) -> Option<Duration> { self.source.total_duration() }
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

/// Anti-click envelope: short fade-in at the start, short fade-out at the end.
/// Prevents pops/clicks from abrupt waveform starts and hard cutoffs.
struct Smooth<S> {
    source: S,
    sample_rate: u32,
    ramp_samples: u64,
    sample_idx: u64,
    total_samples: Option<u64>,
    prev_val: f32, // for DC-blocking / click suppression
}

trait SmoothExt: Source<Item = f32> + Sized {
    fn smooth(self, ramp_ms: u64) -> Smooth<Self>;
}

impl<S: Source<Item = f32>> SmoothExt for S {
    fn smooth(self, ramp_ms: u64) -> Smooth<Self> {
        let sr = self.sample_rate();
        let ramp = (ramp_ms as f32 * sr as f32 / 1000.0) as u64;
        let total = self.total_duration().map(|d| (d.as_secs_f32() * sr as f32) as u64);
        Smooth { source: self, sample_rate: sr, ramp_samples: ramp, sample_idx: 0, total_samples: total, prev_val: 0.0 }
    }
}

impl<S: Source<Item = f32>> Iterator for Smooth<S> {
    type Item = f32;
    fn next(&mut self) -> Option<f32> {
        let val = self.source.next()?;
        let idx = self.sample_idx;
        self.sample_idx += 1;

        // Attack ramp (fade in)
        let attack = if idx < self.ramp_samples {
            idx as f32 / self.ramp_samples.max(1) as f32
        } else {
            1.0
        };

        // Release ramp (fade out at the end) — only if we know total duration
        let release = if let Some(total) = self.total_samples {
            let remaining = total.saturating_sub(idx);
            if remaining < self.ramp_samples {
                remaining as f32 / self.ramp_samples.max(1) as f32
            } else {
                1.0
            }
        } else {
            1.0
        };

        // One-pole lowpass to suppress clicks from abrupt transitions
        let ramped = val * attack * release;
        let smoothed = self.prev_val + 0.3 * (ramped - self.prev_val);
        self.prev_val = smoothed;
        Some(smoothed)
    }
}

impl<S: Source<Item = f32>> Source for Smooth<S> {
    fn current_frame_len(&self) -> Option<usize> { self.source.current_frame_len() }
    fn channels(&self) -> u16 { self.source.channels() }
    fn sample_rate(&self) -> u32 { self.sample_rate }
    fn total_duration(&self) -> Option<Duration> { self.source.total_duration() }
}

trait ThenExt: Source<Item = f32> + Sized + Send + 'static {
    fn then<S: Source<Item = f32> + Send + 'static>(self, other: S) -> Box<dyn Source<Item = f32> + Send>;
}

impl<T: Source<Item = f32> + Send + 'static> ThenExt for T {
    fn then<S: Source<Item = f32> + Send + 'static>(self, other: S) -> Box<dyn Source<Item = f32> + Send> {
        Box::new(ChainSource {
            a: Some(Box::new(self)),
            b: Some(Box::new(other)),
            xfade_samples: 132,  // ~3ms at 44100 Hz
            xfade_idx: 0,
            xfading: false,
            a_tail: Vec::new(),
        })
    }
}

struct ChainSource {
    a: Option<Box<dyn Source<Item = f32> + Send>>,
    b: Option<Box<dyn Source<Item = f32> + Send>>,
    // Anti-click crossfade at the transition point
    xfade_samples: u64,     // ~3ms at 44100 = 132 samples
    xfade_idx: u64,
    xfading: bool,
    a_tail: Vec<f32>,       // buffered tail of source A for crossfading
}

impl Iterator for ChainSource {
    type Item = f32;
    fn next(&mut self) -> Option<f32> {
        // During crossfade: blend A's tail out with B's start in
        if self.xfading {
            let b_val = if let Some(b) = &mut self.b {
                b.next().unwrap_or(0.0)
            } else {
                0.0
            };
            let a_val = if (self.xfade_idx as usize) < self.a_tail.len() {
                self.a_tail[self.xfade_idx as usize]
            } else {
                0.0
            };
            let t = self.xfade_idx as f32 / self.xfade_samples.max(1) as f32;
            self.xfade_idx += 1;
            if self.xfade_idx >= self.xfade_samples {
                self.xfading = false;
            }
            return Some(a_val * (1.0 - t) + b_val * t);
        }

        if let Some(a) = &mut self.a {
            match a.next() {
                Some(v) => return Some(v),
                None => {
                    self.a = None;
                    // Start crossfade into B
                    if self.b.is_some() && self.xfade_samples > 0 {
                        self.xfading = true;
                        self.xfade_idx = 0;
                        // A just ended so its tail is silence — but we still ramp B in
                        return self.next();
                    }
                }
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
