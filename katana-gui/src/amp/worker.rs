//! The worker: owns the MIDI connection and runs one amp job at a time. A plain
//! struct, so tests drive `handle` and `idle` without a thread.

use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender};
use std::time::{Duration, Instant};

use katana_proto::{amp, device, encode, read_patch, read_slot, write_patch, Address, Command, ProtoError, Transport};

use super::{Event, Job};

/// Opens the connection to the amp.
pub type Opener = Box<dyn FnMut() -> Result<Box<dyn Transport + Send>, ProtoError> + Send>;
/// Whether the amp's MIDI port is listed. No amp traffic.
pub type PortCheck = Box<dyn FnMut() -> bool + Send>;

/// How often the port list is checked.
const PORT_CHECK: Duration = Duration::from_secs(1);
/// How long `run` waits for a job before calling `idle`.
const TICK: Duration = Duration::from_millis(20);
/// How long the amp gets to answer one read (as in the CLI).
const REPLY_TIMEOUT: Duration = Duration::from_secs(2);

const NOT_CONNECTED: &str = "amp not connected";

pub struct Worker {
    open: Opener,
    port_present: PortCheck,
    transport: Option<Box<dyn Transport + Send>>,
    live: bool,
    last_port_check: Option<Instant>,
    timeout: Duration,
}

impl Worker {
    pub fn new(open: Opener, port_present: PortCheck) -> Worker {
        Worker { open, port_present, transport: None, live: false, last_port_check: None, timeout: REPLY_TIMEOUT }
    }

    /// Runs `job` and returns everything it reported.
    pub fn handle(&mut self, job: Job) -> Vec<Event> {
        let mut events = Vec::new();
        self.handle_with(job, &mut |e| events.push(e));
        events
    }

    /// Runs `job`, reporting each event as it happens: `Busy(Some(_))` has to
    /// reach the UI before a long job starts, not after it ended.
    pub fn handle_with(&mut self, job: Job, emit: &mut dyn FnMut(Event)) {
        if job == Job::Shutdown {
            return;
        }
        let Some(mut t) = self.transport.take() else {
            emit(Event::Failed(NOT_CONNECTED.into()));
            return;
        };
        let busy = busy_label(&job);
        if let Some(what) = busy {
            emit(Event::Busy(Some(what.into())));
        }
        match self.run_job(t.as_mut(), job) {
            Ok(events) => {
                self.transport = Some(t);
                events.into_iter().for_each(&mut *emit);
            }
            Err(e) if is_link_error(&e) && !(self.port_present)() => {
                self.live = false;
                emit(Event::Disconnected);
            }
            Err(e) => {
                self.transport = Some(t);
                emit(Event::Failed(e.to_string()));
            }
        }
        if busy.is_some() {
            emit(Event::Busy(None));
        }
    }

    fn run_job(&mut self, t: &mut (dyn Transport + Send), job: Job) -> Result<Vec<Event>, ProtoError> {
        Ok(match job {
            Job::LiveOn => {
                amp::set_editor_mode(t, true)?;
                match self.read_current(t) {
                    Ok((sel, p)) => {
                        self.live = true;
                        vec![Event::Current(sel, p)]
                    }
                    Err(e) => {
                        let _ = amp::set_editor_mode(t, false);
                        return Err(e);
                    }
                }
            }
            Job::LiveOff => {
                self.live = false;
                amp::set_editor_mode(t, false)?;
                vec![]
            }
            Job::ReadSlot(sel) => vec![Event::Slot(sel, read_slot(t, sel + 1, self.timeout)?)],
            Job::Select(_) if !self.live => vec![Event::Failed("Live edit is off".into())],
            Job::Select(sel) => {
                amp::select(t, sel, self.timeout)?;
                vec![Event::Current(sel, read_patch(t, self.timeout)?)]
            }
            Job::WritePatch(p) => {
                write_patch(t, &p)?;
                vec![Event::AmpOn(amp::read_selector(t, self.timeout)?)]
            }
            Job::SetParam { addr, data } => {
                t.send(&encode(&device::header(), &Command::Dt1 { addr: Address(addr), data })?)?;
                vec![]
            }
            Job::Peek(ch) => vec![Event::Peeked(ch, read_slot(t, ch.slot(), self.timeout)?)],
            Job::Store { .. } => vec![Event::StoreFailed("Storing is not available yet".into())],
            Job::Shutdown => vec![],
        })
    }

    fn read_current(&self, t: &mut (dyn Transport + Send)) -> Result<(u8, katana_proto::Patch), ProtoError> {
        Ok((amp::read_selector(t, self.timeout)?, read_patch(t, self.timeout)?))
    }

    /// Between jobs: watches the port list, at most once per `PORT_CHECK`.
    pub fn idle(&mut self, now: Instant) -> Vec<Event> {
        if self.last_port_check.is_some_and(|last| now.duration_since(last) < PORT_CHECK) {
            return vec![];
        }
        self.last_port_check = Some(now);
        let present = (self.port_present)();
        match (self.transport.is_some(), present) {
            (true, false) => {
                self.transport = None;
                self.live = false;
                vec![Event::Disconnected]
            }
            (false, true) => self.connect(),
            _ => vec![],
        }
    }

    /// Opens the port. An amp that can't be opened or doesn't answer stays "not
    /// connected"; the next port check tries again.
    fn connect(&mut self) -> Vec<Event> {
        let Ok(mut t) = (self.open)() else { return vec![] };
        // Clears editor mode left on by a crash.
        if amp::set_editor_mode(t.as_mut(), false).is_err() {
            return vec![];
        }
        let Ok(sel) = amp::read_selector(t.as_mut(), self.timeout) else { return vec![] };
        self.transport = Some(t);
        vec![Event::Connected, Event::AmpOn(sel)]
    }

    /// The thread's loop: jobs first, `idle` when there are none. Ends on
    /// `Shutdown` or when the UI's end of the channel is gone.
    pub fn run(mut self, jobs: Receiver<Job>, events: Sender<Event>, wake: Box<dyn Fn() + Send>) {
        let mut emit = |e: Event| {
            let _ = events.send(e);
            wake();
        };
        loop {
            let mut batch = Vec::new();
            match jobs.recv_timeout(TICK) {
                Ok(job) => batch.push(job),
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => batch.push(Job::Shutdown),
            }
            batch.extend(jobs.try_iter());
            if batch.is_empty() {
                self.idle(Instant::now()).into_iter().for_each(&mut emit);
                continue;
            }
            for job in coalesce(batch) {
                let last = job == Job::Shutdown;
                self.handle_with(job, &mut emit);
                if last {
                    return;
                }
            }
        }
    }
}

/// Text for the UI while a long job runs; short jobs have none.
fn busy_label(job: &Job) -> Option<&'static str> {
    match job {
        Job::LiveOn => Some("Turning Live edit on\u{2026}"),
        Job::ReadSlot(_) | Job::Peek(_) => Some("Reading channel\u{2026}"),
        Job::Select(_) => Some("Switching channel\u{2026}"),
        Job::WritePatch(_) => Some("Loading to amp\u{2026}"),
        Job::Store { .. } => Some("Storing\u{2026}"),
        Job::LiveOff | Job::SetParam { .. } | Job::Shutdown => None,
    }
}

/// An error that an unplugged amp would cause.
fn is_link_error(e: &ProtoError) -> bool {
    matches!(e, ProtoError::Midi(_) | ProtoError::Timeout { .. })
}

/// Drops each `SetParam` that a later one to the same address replaces, as long
/// as only `SetParam`s lie between them: a fast knob drag sends the last value.
pub fn coalesce(jobs: Vec<Job>) -> Vec<Job> {
    let mut out: Vec<Job> = Vec::with_capacity(jobs.len());
    let mut run_start = 0;
    for job in jobs {
        if let Job::SetParam { addr, .. } = &job {
            let replaced =
                out[run_start..].iter().position(|j| matches!(j, Job::SetParam { addr: a, .. } if a == addr));
            if let Some(i) = replaced {
                out.remove(run_start + i);
            }
            out.push(job);
        } else {
            out.push(job);
            run_start = out.len();
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};

    use katana_proto::device::{self, PATCH_BLOCKS};
    use katana_proto::patch::slot_offset;
    use katana_proto::{amp, decode, encode, read_patch, read_slot, Address, Channel, Command, FakeAmp, Patch};

    use super::*;

    const T: Duration = Duration::from_millis(50);

    struct SharedFake(Arc<Mutex<FakeAmp>>);

    impl Transport for SharedFake {
        fn send(&mut self, msg: &[u8]) -> Result<(), ProtoError> {
            self.0.lock().unwrap().send(msg)
        }

        fn recv(&mut self, timeout: Duration) -> Result<Option<Vec<u8>>, ProtoError> {
            self.0.lock().unwrap().recv(timeout)
        }
    }

    /// Current patch filled with 0x40, stored slot n with n, selector at A4.
    fn fake_amp() -> Arc<Mutex<FakeAmp>> {
        let mut amp = FakeAmp::new(device::header(), 128);
        for &(a, s) in PATCH_BLOCKS {
            amp.mem.apply(Address(a), &vec![0x40; s as usize]);
            for n in 1..=9u8 {
                amp.mem.apply(Address(a + slot_offset(n)), &vec![n; s as usize]);
            }
        }
        amp.mem.apply(Address(device::SELECTOR), &[0, 4]);
        Arc::new(Mutex::new(amp))
    }

    struct Rig {
        worker: Worker,
        fake: Arc<Mutex<FakeAmp>>,
        port: Arc<AtomicBool>,
        checks: Arc<AtomicUsize>,
        opens: Arc<AtomicUsize>,
    }

    fn rig(port_present: bool) -> Rig {
        let fake = fake_amp();
        let port = Arc::new(AtomicBool::new(port_present));
        let checks = Arc::new(AtomicUsize::new(0));
        let opens = Arc::new(AtomicUsize::new(0));
        let (f, o, p, c) = (fake.clone(), opens.clone(), port.clone(), checks.clone());
        let mut worker = Worker::new(
            Box::new(move || {
                o.fetch_add(1, Ordering::SeqCst);
                Ok(Box::new(SharedFake(f.clone())) as Box<dyn Transport + Send>)
            }),
            Box::new(move || {
                c.fetch_add(1, Ordering::SeqCst);
                p.load(Ordering::SeqCst)
            }),
        );
        worker.timeout = T;
        Rig { worker, fake, port, checks, opens }
    }

    /// A worker already connected to the fake; the fake's log is cleared.
    fn connected() -> Rig {
        let mut r = rig(true);
        assert!(r.worker.idle(Instant::now()).contains(&Event::Connected));
        r.fake.lock().unwrap().sent.clear();
        r
    }

    fn current_of(fake: &Arc<Mutex<FakeAmp>>) -> Patch {
        read_patch(&mut *fake.lock().unwrap(), T).unwrap()
    }

    fn slot_of(fake: &Arc<Mutex<FakeAmp>>, slot: u8) -> Patch {
        read_slot(&mut *fake.lock().unwrap(), slot, T).unwrap()
    }

    fn selector_of(fake: &Arc<Mutex<FakeAmp>>) -> u8 {
        amp::read_selector(&mut *fake.lock().unwrap(), T).unwrap()
    }

    /// Without the `Busy` wrapping.
    fn outcome(events: Vec<Event>) -> Vec<Event> {
        events.into_iter().filter(|e| !matches!(e, Event::Busy(_))).collect()
    }

    fn dt1_count(fake: &Arc<Mutex<FakeAmp>>) -> usize {
        let amp = fake.lock().unwrap();
        amp.sent.iter().filter(|m| matches!(decode(&amp.header, m), Ok(Command::Dt1 { .. }))).count()
    }

    fn dt1(addr: u32, data: &[u8]) -> Vec<u8> {
        encode(&device::header(), &Command::Dt1 { addr: Address(addr), data: data.to_vec() }).unwrap()
    }

    #[test]
    fn connects_when_port_appears() {
        let mut r = rig(false);
        let t0 = Instant::now();
        assert_eq!(r.worker.idle(t0), vec![]);
        assert_eq!(r.opens.load(Ordering::SeqCst), 0);

        r.port.store(true, Ordering::SeqCst);
        r.fake.lock().unwrap().editor = true; // left on by a crash
        assert_eq!(r.worker.idle(t0 + Duration::from_millis(1100)), vec![Event::Connected, Event::AmpOn(4)]);
        let amp = r.fake.lock().unwrap();
        assert!(!amp.editor);
        assert_eq!(amp.sent[0], dt1(device::EDITOR_MODE, &[0]));
    }

    #[test]
    fn checks_the_port_once_a_second() {
        let mut r = rig(false);
        let t0 = Instant::now();
        r.worker.idle(t0);
        r.worker.idle(t0 + Duration::from_millis(500));
        assert_eq!(r.checks.load(Ordering::SeqCst), 1);
        r.worker.idle(t0 + Duration::from_millis(1100));
        assert_eq!(r.checks.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn failed_open_stays_disconnected_and_retries() {
        let opens = Arc::new(AtomicUsize::new(0));
        let o = opens.clone();
        let mut w = Worker::new(
            Box::new(move || {
                o.fetch_add(1, Ordering::SeqCst);
                Err(ProtoError::Midi("port in use".into()))
            }),
            Box::new(|| true),
        );
        let t0 = Instant::now();
        assert_eq!(w.idle(t0), vec![]);
        assert_eq!(w.idle(t0 + Duration::from_millis(1100)), vec![]);
        assert_eq!(opens.load(Ordering::SeqCst), 2);
        assert_eq!(w.handle(Job::LiveOn), vec![Event::Failed("amp not connected".into())]);
    }

    #[test]
    fn silent_amp_at_connect_stays_disconnected() {
        let mut r = rig(true);
        r.fake.lock().unwrap().silent = true;
        assert_eq!(r.worker.idle(Instant::now()), vec![]);
        assert_eq!(r.worker.handle(Job::LiveOn), vec![Event::Failed("amp not connected".into())]);
    }

    #[test]
    fn port_gone_while_idle_reports_disconnect() {
        let mut r = connected();
        r.port.store(false, Ordering::SeqCst);
        let later = Instant::now() + Duration::from_millis(1100);
        assert_eq!(r.worker.idle(later), vec![Event::Disconnected]);
        assert_eq!(r.worker.handle(Job::ReadSlot(1)), vec![Event::Failed("amp not connected".into())]);
    }

    #[test]
    fn reconnects_after_disconnect() {
        let mut r = connected();
        let t0 = Instant::now();
        r.port.store(false, Ordering::SeqCst);
        assert_eq!(r.worker.idle(t0 + Duration::from_millis(1100)), vec![Event::Disconnected]);
        r.port.store(true, Ordering::SeqCst);
        assert_eq!(r.worker.idle(t0 + Duration::from_millis(2200)), vec![Event::Connected, Event::AmpOn(4)]);
    }

    #[test]
    fn unplug_mid_job_reports_disconnect() {
        let mut r = connected();
        r.fake.lock().unwrap().silent = true;
        r.port.store(false, Ordering::SeqCst);
        assert_eq!(outcome(r.worker.handle(Job::ReadSlot(3))), vec![Event::Disconnected]);
        assert_eq!(r.worker.handle(Job::ReadSlot(3)), vec![Event::Failed("amp not connected".into())]);
    }

    #[test]
    fn timeout_with_port_present_is_a_failure_not_a_disconnect() {
        let mut r = connected();
        r.fake.lock().unwrap().silent = true;
        let events = outcome(r.worker.handle(Job::ReadSlot(3)));
        assert!(matches!(events.as_slice(), [Event::Failed(m)] if m.contains("no complete reply")), "{events:?}");
        r.fake.lock().unwrap().silent = false;
        assert!(matches!(outcome(r.worker.handle(Job::ReadSlot(3))).as_slice(), [Event::Slot(3, _)]));
    }

    #[test]
    fn job_without_amp_fails() {
        let mut r = rig(false);
        assert_eq!(r.worker.handle(Job::LiveOn), vec![Event::Failed("amp not connected".into())]);
    }

    #[test]
    fn live_on_reads_current() {
        let mut r = connected();
        let p = current_of(&r.fake);
        assert_eq!(outcome(r.worker.handle(Job::LiveOn)), vec![Event::Current(4, p)]);
        assert!(r.fake.lock().unwrap().editor);
    }

    #[test]
    fn live_on_failure_leaves_editor_mode_off() {
        let mut r = connected();
        r.fake.lock().unwrap().silent = true;
        assert!(matches!(outcome(r.worker.handle(Job::LiveOn)).as_slice(), [Event::Failed(_)]));
        assert!(!r.fake.lock().unwrap().editor);
    }

    #[test]
    fn live_off_turns_editor_mode_off() {
        let mut r = connected();
        r.worker.handle(Job::LiveOn);
        assert_eq!(r.worker.handle(Job::LiveOff), vec![]);
        assert!(!r.fake.lock().unwrap().editor);
    }

    #[test]
    fn read_slot_is_view_only() {
        let mut r = connected();
        let expected = slot_of(&r.fake, 3);
        r.fake.lock().unwrap().sent.clear();
        assert_eq!(outcome(r.worker.handle(Job::ReadSlot(2))), vec![Event::Slot(2, expected)]);
        assert_eq!(selector_of(&r.fake), 4);
        assert_eq!(dt1_count(&r.fake), 0);
    }

    #[test]
    fn select_with_live_on_switches_and_reads() {
        let mut r = connected();
        r.worker.handle(Job::LiveOn);
        let expected = slot_of(&r.fake, 3);
        assert_eq!(outcome(r.worker.handle(Job::Select(2))), vec![Event::Current(2, expected)]);
        assert_eq!(selector_of(&r.fake), 2);
    }

    #[test]
    fn select_with_live_off_fails_without_touching_the_amp() {
        let mut r = connected();
        assert!(matches!(outcome(r.worker.handle(Job::Select(2))).as_slice(), [Event::Failed(_)]));
        assert_eq!(dt1_count(&r.fake), 0);
    }

    #[test]
    fn write_patch_reports_channel() {
        let mut r = connected();
        let mut p = Patch::default_preset();
        p.set_name("Loaded");
        assert_eq!(outcome(r.worker.handle(Job::WritePatch(p))), vec![Event::AmpOn(4)]);
        assert_eq!(current_of(&r.fake).name(), "Loaded");
    }

    #[test]
    fn set_param_sends_one_dt1_and_no_busy() {
        let mut r = connected();
        let a = device::addr([0x20, 0x00, 0x06, 0x00]); // GAIN
        assert_eq!(r.worker.handle(Job::SetParam { addr: a, data: vec![9] }), vec![]);
        assert_eq!(r.fake.lock().unwrap().sent, vec![dt1(a, &[9])]);
    }

    #[test]
    fn long_jobs_are_wrapped_in_busy() {
        let mut r = connected();
        let events = r.worker.handle(Job::ReadSlot(2));
        assert!(matches!(events.first(), Some(Event::Busy(Some(_)))), "{events:?}");
        assert_eq!(events.last(), Some(&Event::Busy(None)));
    }

    #[test]
    fn busy_is_reported_before_the_job_runs() {
        let mut r = connected();
        let fake = r.fake.clone();
        let mut sent_at_busy = None;
        r.worker.handle_with(Job::ReadSlot(2), &mut |e| {
            if matches!(e, Event::Busy(Some(_))) {
                sent_at_busy = Some(fake.lock().unwrap().sent.len());
            }
        });
        assert_eq!(sent_at_busy, Some(0));
    }

    fn set(addr: u32, v: u8) -> Job {
        Job::SetParam { addr, data: vec![v] }
    }

    #[test]
    fn coalesce_keeps_last_value_per_address() {
        let (a, b) = (1, 2);
        let wp = Job::WritePatch(Patch::default());
        assert_eq!(
            coalesce(vec![set(a, 1), set(b, 1), set(a, 2), wp.clone(), set(a, 3)]),
            vec![set(b, 1), set(a, 2), wp, set(a, 3)]
        );
    }

    #[test]
    fn coalesce_leaves_other_jobs_alone() {
        let jobs = vec![Job::LiveOn, Job::ReadSlot(1), Job::Peek(Channel::A1), Job::LiveOff];
        assert_eq!(coalesce(jobs.clone()), jobs);
    }
}
