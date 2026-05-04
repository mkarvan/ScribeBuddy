use crossbeam_channel;
use scribebuddy_core::{
    audio::{
        capture::{make_audio_ring_buffer, push_audio_buffer, AudioBuffer, AudioConsumer, AudioSource},
        processor::AudioProcessor,
    },
    transcription::Transcriber,
    SessionConfig, Speaker, TranscriptSegment,
};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;

// ── MockAudioSource ──────────────────────────────────────────────────────────

struct MockAudioSource {
    name: String,
    rate: u32,
    samples: Vec<f32>,
    consumer: Option<AudioConsumer>,
}

impl MockAudioSource {
    fn new(name: &str, rate: u32, samples: Vec<f32>) -> Self {
        Self { name: name.to_string(), rate, samples, consumer: None }
    }
}

impl AudioSource for MockAudioSource {
    fn start(&mut self) -> anyhow::Result<()> {
        let (mut prod, cons) = make_audio_ring_buffer(4);
        push_audio_buffer(&mut prod, AudioBuffer {
            data: self.samples.clone(),
            timestamp: std::time::Instant::now(),
        });
        self.consumer = Some(cons);
        Ok(())
    }
    fn stop(&mut self) -> anyhow::Result<()> { Ok(()) }
    fn sample_rate(&self) -> u32 { self.rate }
    fn channels(&self) -> u16 { 1 }
    fn name(&self) -> &str { &self.name }
    fn take_consumer(&mut self) -> Option<AudioConsumer> { self.consumer.take() }
}

// ── MockTranscriber ───────────────────────────────────────────────────────────

struct MockTranscriber {
    calls: Arc<AtomicUsize>,
    running: Arc<AtomicBool>,
    stop_after: usize,
}

impl MockTranscriber {
    fn new(running: Arc<AtomicBool>, stop_after: usize) -> (Self, Arc<AtomicUsize>) {
        let calls = Arc::new(AtomicUsize::new(0));
        let mock = Self { calls: calls.clone(), running, stop_after };
        (mock, calls)
    }
}

impl Transcriber for MockTranscriber {
    fn transcribe(
        &mut self,
        _samples: &[f32],
        speaker: Speaker,
        offset_secs: i64,
    ) -> anyhow::Result<Vec<TranscriptSegment>> {
        let n = self.calls.fetch_add(1, Ordering::SeqCst) + 1;
        if n >= self.stop_after {
            self.running.store(false, Ordering::SeqCst);
        }
        Ok(vec![TranscriptSegment::new(
            speaker,
            format!("chunk {}", n),
            offset_secs,
            offset_secs + 1,
        )])
    }
}

// ── helpers ───────────────────────────────────────────────────────────────────

const SOURCE_RATE: u32 = 48000;

fn make_config(chunk_secs: f32) -> SessionConfig {
    SessionConfig {
        chunk_duration_secs: chunk_secs,
        silence_threshold_rms: 0.003,
        ..SessionConfig::default()
    }
}

/// Generate `n_chunks` chunks worth of samples at `SOURCE_RATE` with the given amplitude.
fn audio_of(n_chunks: usize, chunk_secs: f32, amplitude: f32) -> Vec<f32> {
    let n = (n_chunks as f32 * chunk_secs * SOURCE_RATE as f32) as usize;
    vec![amplitude; n]
}

// ── tests ─────────────────────────────────────────────────────────────────────

/// Silent audio (amplitude 0.0) must never reach the transcriber.
#[test]
fn silence_gate_suppresses_transcriber() {
    let chunk_secs = 0.1f32;
    let running = Arc::new(AtomicBool::new(true));
    let (seg_tx, seg_rx) = crossbeam_channel::bounded::<TranscriptSegment>(32);
    let (err_tx, _err_rx) = crossbeam_channel::bounded::<String>(8);

    let (mock, call_count) = MockTranscriber::new(running.clone(), usize::MAX);

    let silence = audio_of(5, chunk_secs, 0.0);
    let mut you = MockAudioSource::new("you", SOURCE_RATE, silence.clone());
    let mut remote = MockAudioSource::new("remote", SOURCE_RATE, silence);
    you.start().unwrap();
    remote.start().unwrap();
    let you_cons = you.take_consumer().unwrap();
    let remote_cons = remote.take_consumer().unwrap();

    // Stop the processor after 300 ms (it will be sleeping since all chunks are silent).
    let stopper = running.clone();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(300));
        stopper.store(false, Ordering::SeqCst);
    });

    let processor = AudioProcessor::new(make_config(chunk_secs), running.clone(), seg_tx, err_tx);
    processor.run_with(you_cons, remote_cons, SOURCE_RATE, SOURCE_RATE, mock);

    assert_eq!(
        call_count.load(Ordering::SeqCst),
        0,
        "transcriber must not be called for silent audio"
    );
    assert!(seg_rx.is_empty(), "no segments should be emitted for silent audio");
}

/// Non-silent audio must reach the transcriber and the resulting segment must
/// flow through the channel back to the caller.
#[test]
fn non_silent_audio_produces_segments() {
    let chunk_secs = 0.1f32;
    let running = Arc::new(AtomicBool::new(true));
    let (seg_tx, seg_rx) = crossbeam_channel::bounded::<TranscriptSegment>(32);
    let (err_tx, _err_rx) = crossbeam_channel::bounded::<String>(8);

    // Stop the processor after the first transcriber call.
    let (mock, call_count) = MockTranscriber::new(running.clone(), 1);

    // Loud you (0.5 >> threshold 0.003); silent remote.
    let loud = audio_of(5, chunk_secs, 0.5);
    let silence = audio_of(5, chunk_secs, 0.0);
    let mut you = MockAudioSource::new("you", SOURCE_RATE, loud);
    let mut remote = MockAudioSource::new("remote", SOURCE_RATE, silence);
    you.start().unwrap();
    remote.start().unwrap();
    let you_cons = you.take_consumer().unwrap();
    let remote_cons = remote.take_consumer().unwrap();

    let processor = AudioProcessor::new(make_config(chunk_secs), running.clone(), seg_tx, err_tx);
    processor.run_with(you_cons, remote_cons, SOURCE_RATE, SOURCE_RATE, mock);

    assert!(
        call_count.load(Ordering::SeqCst) >= 1,
        "transcriber must be called for loud audio"
    );

    let received: Vec<_> = seg_rx.try_iter().collect();
    assert!(!received.is_empty(), "at least one segment must reach the channel");
    assert_eq!(received[0].speaker, Speaker::You, "segment speaker must be You");
}
