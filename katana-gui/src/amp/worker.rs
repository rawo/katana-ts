//! The worker: owns the MIDI connection and runs one amp job at a time. A plain
//! struct, so tests drive `handle` and `idle` without a thread.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender};
use std::time::{Duration, Instant};

use katana_proto::amp::{notification, Notification};
use katana_proto::backup::{backup_name, write_backup};
use katana_proto::{
    amp, device, encode, read_blocks, read_patch, read_slot, write_patch, Address, Channel, Command, Patch, ProtoError,
    Transport,
};

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

/// How often `device::PANEL_BLOCKS` are re-read while Live is on.
const PANEL_REREAD: Duration = Duration::from_secs(1);
/// Amp messages handled per `idle`, so a knob being turned can't hold up jobs.
const MAX_MESSAGES_PER_IDLE: usize = 64;

/// Whether `a` is in the current patch, where a panel change lands.
fn in_current_patch(a: u32) -> bool {
    (device::addr([0x20, 0x00, 0x00, 0x00])..=device::addr([0x20, 0x01, 0x7F, 0x7F])).contains(&a)
}

/// What a failed store tells the user: it may have left part of the new preset
/// playing, and where the previous sounds are.
pub fn store_failure_message(ch: Channel, error: &str, channel_backup: &Path, current_backup: &Path) -> String {
    format!(
        "Storing into {ch} failed: {error}. The amp now plays {ch} with part of the new preset. \
         Your previous sound: {} (Open it, then Load to amp). {ch}'s backup: {}.",
        current_backup.display(),
        channel_backup.display()
    )
}

/// Backs up the channel about to be overwritten and the current sound.
fn write_store_backups(
    dir: &Path,
    stamp: &str,
    ch: Channel,
    slot: &Patch,
    current: &Patch,
) -> std::io::Result<(PathBuf, PathBuf)> {
    std::fs::create_dir_all(dir)?;
    let channel_backup = write_backup(dir, &backup_name(stamp, &ch.to_string(), &slot.name()), slot)?;
    let current_backup = write_backup(dir, &backup_name(stamp, "current", &current.name()), current)?;
    Ok((channel_backup, current_backup))
}

pub struct Worker {
    open: Opener,
    port_present: PortCheck,
    transport: Option<Box<dyn Transport + Send>>,
    live: bool,
    last_port_check: Option<Instant>,
    /// The amp's current settings as last read or written here, while Live is on.
    shadow: Option<Patch>,
    last_reread: Option<Instant>,
    /// A store lost the amp: `handle_with` ends the connection after its events.
    link_lost: bool,
    timeout: Duration,
    panel_blocks: &'static [(u32, u32)],
}

impl Worker {
    pub fn new(open: Opener, port_present: PortCheck) -> Worker {
        Worker {
            open,
            port_present,
            transport: None,
            live: false,
            last_port_check: None,
            shadow: None,
            last_reread: None,
            link_lost: false,
            timeout: REPLY_TIMEOUT,
            panel_blocks: device::PANEL_BLOCKS,
        }
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
            self.turn_editor_mode_off();
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
                events.into_iter().for_each(&mut *emit);
                if std::mem::take(&mut self.link_lost) {
                    self.end_live();
                    emit(Event::Disconnected);
                } else {
                    self.transport = Some(t);
                }
            }
            Err(e) => self.after_error(t, e).into_iter().for_each(&mut *emit),
        }
        if busy.is_some() {
            emit(Event::Busy(None));
        }
    }

    /// A failed job: an unplugged amp ends the connection, anything else is
    /// reported and the connection stays.
    fn after_error(&mut self, t: Box<dyn Transport + Send>, e: ProtoError) -> Vec<Event> {
        if is_link_error(&e) && !(self.port_present)() {
            self.end_live();
            vec![Event::Disconnected]
        } else {
            self.transport = Some(t);
            vec![Event::Failed(e.to_string())]
        }
    }

    fn end_live(&mut self) {
        self.live = false;
        self.shadow = None;
    }

    fn turn_editor_mode_off(&mut self) {
        if let Some(mut t) = self.transport.take() {
            let _ = amp::set_editor_mode(t.as_mut(), false);
        }
    }

    fn run_job(&mut self, t: &mut (dyn Transport + Send), job: Job) -> Result<Vec<Event>, ProtoError> {
        Ok(match job {
            Job::LiveOn => {
                amp::set_editor_mode(t, true)?;
                match self.read_current(t) {
                    Ok((sel, p)) => {
                        self.live = true;
                        self.shadow = Some(p.clone());
                        self.last_reread = None;
                        vec![Event::Current(sel, p)]
                    }
                    Err(e) => {
                        let _ = amp::set_editor_mode(t, false);
                        return Err(e);
                    }
                }
            }
            Job::LiveOff => {
                self.end_live();
                amp::set_editor_mode(t, false)?;
                vec![]
            }
            Job::ReadSlot(sel) => vec![Event::Slot(sel, read_slot(t, sel + 1, self.timeout)?)],
            Job::Select(_) if !self.live => vec![Event::Failed("Live edit is off".into())],
            Job::Select(sel) => {
                amp::select(t, sel, self.timeout)?;
                let p = read_patch(t, self.timeout)?;
                self.shadow = Some(p.clone());
                vec![Event::Current(sel, p)]
            }
            Job::WritePatch(p) => {
                write_patch(t, &p)?;
                if let Some(shadow) = &mut self.shadow {
                    let written = p.mem.0.iter().filter(|(&k, _)| !device::is_write_protected(k));
                    shadow.mem.0.extend(written.map(|(&k, &b)| (k, b)));
                }
                vec![Event::AmpOn(amp::read_selector(t, self.timeout)?)]
            }
            Job::SetParam { addr, data } => {
                t.send(&encode(&device::header(), &Command::Dt1 { addr: Address(addr), data: data.clone() })?)?;
                if let Some(shadow) = &mut self.shadow {
                    shadow.mem.apply(Address(addr), &data);
                }
                vec![]
            }
            Job::Peek(ch) => vec![Event::Peeked(ch, read_slot(t, ch.slot(), self.timeout)?)],
            Job::Store { ch, patch, backup_dir, stamp } => self.store(t, ch, &patch, &backup_dir, &stamp)?,
            Job::Shutdown => vec![],
        })
    }

    fn read_current(&self, t: &mut (dyn Transport + Send)) -> Result<(u8, Patch), ProtoError> {
        Ok((amp::read_selector(t, self.timeout)?, read_patch(t, self.timeout)?))
    }

    /// Backups first, then the store. `store_patch` ends with editor mode off, so
    /// Live is turned back on afterwards.
    fn store(
        &mut self,
        t: &mut (dyn Transport + Send),
        ch: Channel,
        patch: &Patch,
        backup_dir: &Path,
        stamp: &str,
    ) -> Result<Vec<Event>, ProtoError> {
        let slot = read_slot(t, ch.slot(), self.timeout)?;
        let current = read_patch(t, self.timeout)?;
        let (channel_backup, current_backup) = match write_store_backups(backup_dir, stamp, ch, &slot, &current) {
            Ok(paths) => paths,
            Err(e) => {
                return Ok(vec![Event::StoreFailed(format!("Couldn't write the backup: {e}; nothing was stored"))]);
            }
        };
        let mut events = match amp::store_patch(t, ch, patch, self.timeout) {
            Ok(report) => vec![
                Event::Stored { ch, channel_backup, current_backup, solo_delay_stored: report.solo_delay_stored },
                Event::AmpOn(ch.selector()),
            ],
            Err(e) => {
                self.link_lost = is_link_error(&e) && !(self.port_present)();
                vec![Event::StoreFailed(store_failure_message(ch, &e.to_string(), &channel_backup, &current_backup))]
            }
        };
        if self.live && !self.link_lost {
            let resumed = amp::set_editor_mode(t, true).and_then(|()| read_patch(t, self.timeout));
            match resumed {
                Ok(p) => self.shadow = Some(p),
                Err(e) if is_link_error(&e) && !(self.port_present)() => self.link_lost = true,
                Err(e) => events.push(Event::Failed(e.to_string())),
            }
        }
        Ok(events)
    }

    /// Between jobs: watches the port list, at most once per `PORT_CHECK`, and
    /// follows the amp while Live is on.
    pub fn idle(&mut self, now: Instant) -> Vec<Event> {
        let mut events = self.check_port(now);
        if let Some(mut t) = self.transport.take() {
            match self.follow_amp(t.as_mut(), now) {
                Ok(more) => {
                    self.transport = Some(t);
                    events.extend(more);
                }
                Err(e) => events.extend(self.after_error(t, e)),
            }
        }
        events
    }

    fn check_port(&mut self, now: Instant) -> Vec<Event> {
        if self.last_port_check.is_some_and(|last| now.duration_since(last) < PORT_CHECK) {
            return vec![];
        }
        self.last_port_check = Some(now);
        let present = (self.port_present)();
        match (self.transport.is_some(), present) {
            (true, false) => {
                self.transport = None;
                self.end_live();
                vec![Event::Disconnected]
            }
            (false, true) => self.connect(),
            _ => vec![],
        }
    }

    /// Handles what the amp sent by itself (ignored with Live off), then the
    /// timed re-read of panel blocks.
    fn follow_amp(&mut self, t: &mut (dyn Transport + Send), now: Instant) -> Result<Vec<Event>, ProtoError> {
        let h = device::header();
        let mut events = Vec::new();
        for _ in 0..MAX_MESSAGES_PER_IDLE {
            let Some(msg) = t.recv(Duration::ZERO)? else { break };
            if !self.live {
                continue;
            }
            match notification(&h, &msg) {
                Some(Notification::Selector(sel)) => {
                    // The amp's own load of the channel must finish first.
                    std::thread::sleep(device::SELECT_SETTLE);
                    let p = read_patch(t, self.timeout)?;
                    self.shadow = Some(p.clone());
                    events.push(Event::Current(sel, p));
                }
                Some(Notification::Other { addr, data }) if in_current_patch(addr.0) => {
                    if let Some(shadow) = &mut self.shadow {
                        shadow.mem.apply(addr, &data);
                    }
                    events.push(Event::PanelChange { addr: addr.0, data });
                }
                _ => {}
            }
        }
        if self.live && !self.panel_blocks.is_empty() {
            match self.last_reread {
                None => self.last_reread = Some(now),
                Some(last) if now.duration_since(last) >= PANEL_REREAD => {
                    self.last_reread = Some(now);
                    events.extend(self.reread_panel_blocks(t)?);
                }
                Some(_) => {}
            }
        }
        Ok(events)
    }

    /// One `PanelChange` per contiguous run of bytes differing from the shadow.
    fn reread_panel_blocks(&mut self, t: &mut (dyn Transport + Send)) -> Result<Vec<Event>, ProtoError> {
        let got = read_blocks(t, self.panel_blocks, self.timeout)?;
        let shadow = self.shadow.get_or_insert_with(Patch::default);
        let mut events: Vec<Event> = Vec::new();
        for (&k, &b) in &got.mem.0 {
            if shadow.mem.0.insert(k, b) == Some(b) {
                continue;
            }
            match events.last_mut() {
                Some(Event::PanelChange { addr, data }) if *addr + data.len() as u32 == k => data.push(b),
                _ => events.push(Event::PanelChange { addr: k, data: vec![b] }),
            }
        }
        Ok(events)
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

impl Drop for Worker {
    fn drop(&mut self) {
        self.turn_editor_mode_off();
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
    use katana_proto::{amp, decode, encode, read_patch, read_slot, Address, Command, FakeAmp, Patch};

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

    const GAIN: u32 = device::addr([0x20, 0x00, 0x06, 0x00]);
    /// `device::PANEL_BLOCKS` is empty (the amp reports every panel action), so
    /// the re-read tests use this list: the AMP block.
    const TEST_PANEL_BLOCKS: &[(u32, u32)] = &[(GAIN, 10)];

    fn live() -> Rig {
        let mut r = connected();
        r.worker.handle(Job::LiveOn);
        r.fake.lock().unwrap().sent.clear();
        r
    }

    fn rq1_count(fake: &Arc<Mutex<FakeAmp>>) -> usize {
        let amp = fake.lock().unwrap();
        amp.sent.iter().filter(|m| matches!(decode(&amp.header, m), Ok(Command::Rq1 { .. }))).count()
    }

    #[test]
    fn panel_dt1_becomes_panel_change() {
        let mut r = live();
        r.fake.lock().unwrap().queue(dt1(GAIN, &[50]));
        assert_eq!(r.worker.idle(Instant::now()), vec![Event::PanelChange { addr: GAIN, data: vec![50] }]);
    }

    #[test]
    fn amp_messages_outside_the_current_patch_are_not_panel_changes() {
        let mut r = live();
        {
            let mut amp = r.fake.lock().unwrap();
            amp.queue(dt1(device::addr([0x7F, 0x01, 0x01, 0x01]), &[3])); // effect LED
            amp.queue(dt1(device::addr([0x30, 0x00, 0x02, 0x00]), &[9])); // knob mirror
        }
        assert_eq!(r.worker.idle(Instant::now()), vec![]);
    }

    #[test]
    fn channel_switch_on_amp_rereads() {
        let mut r = live();
        let expected = slot_of(&r.fake, 7);
        // The amp switches and sends the selector, as the panel's channel button does.
        r.fake.lock().unwrap().send(&dt1(device::PATCH_SELECT, &[0, 6])).unwrap();
        assert_eq!(r.worker.idle(Instant::now()), vec![Event::Current(6, expected)]);
    }

    #[test]
    fn rereads_panel_blocks_once_a_second() {
        let mut r = live();
        r.worker.panel_blocks = TEST_PANEL_BLOCKS;
        let t0 = Instant::now();
        r.fake.lock().unwrap().mem.apply(Address(GAIN), &[77]);
        assert_eq!(r.worker.idle(t0), vec![]);
        assert_eq!(rq1_count(&r.fake), 0);
        let events = r.worker.idle(t0 + Duration::from_millis(1100));
        assert_eq!(events, vec![Event::PanelChange { addr: GAIN, data: vec![77] }]);
        assert_eq!(rq1_count(&r.fake), TEST_PANEL_BLOCKS.len());
        // Reported once: the shadow has it now.
        assert_eq!(r.worker.idle(t0 + Duration::from_millis(2200)), vec![]);
    }

    #[test]
    fn reread_joins_neighbouring_bytes_into_one_change() {
        let mut r = live();
        r.worker.panel_blocks = TEST_PANEL_BLOCKS;
        let t0 = Instant::now();
        r.worker.idle(t0);
        {
            let mut amp = r.fake.lock().unwrap();
            amp.mem.apply(Address(GAIN), &[1, 2]);
            amp.mem.apply(Address(GAIN + 5), &[3]);
        }
        assert_eq!(
            r.worker.idle(t0 + Duration::from_millis(1100)),
            vec![
                Event::PanelChange { addr: GAIN, data: vec![1, 2] },
                Event::PanelChange { addr: GAIN + 5, data: vec![3] }
            ]
        );
    }

    #[test]
    fn own_writes_are_not_panel_changes() {
        let mut r = live();
        r.worker.panel_blocks = TEST_PANEL_BLOCKS;
        let t0 = Instant::now();
        r.worker.idle(t0);
        r.worker.handle(Job::SetParam { addr: GAIN, data: vec![70] });
        assert_eq!(r.worker.idle(t0 + Duration::from_millis(1100)), vec![]);
    }

    #[test]
    fn own_patch_load_is_not_a_panel_change() {
        let mut r = live();
        r.worker.panel_blocks = TEST_PANEL_BLOCKS;
        let t0 = Instant::now();
        r.worker.idle(t0);
        let mut p = Patch::default_preset();
        p.mem.apply(Address(GAIN), &[71]);
        r.worker.handle(Job::WritePatch(p));
        assert_eq!(r.worker.idle(t0 + Duration::from_millis(1100)), vec![]);
    }

    #[test]
    fn no_rereads_with_live_off() {
        let mut r = connected();
        r.worker.panel_blocks = TEST_PANEL_BLOCKS;
        let t0 = Instant::now();
        r.worker.idle(t0);
        r.worker.idle(t0 + Duration::from_millis(1100));
        assert_eq!(rq1_count(&r.fake), 0);
    }

    #[test]
    fn live_off_ignores_amp_messages() {
        let mut r = connected();
        r.fake.lock().unwrap().queue(dt1(GAIN, &[50]));
        assert_eq!(r.worker.idle(Instant::now()), vec![]);
        // Drained: it doesn't turn up once Live is on.
        r.worker.handle(Job::LiveOn);
        assert_eq!(r.worker.idle(Instant::now()), vec![]);
    }

    #[test]
    fn live_ends_with_live_off() {
        let mut r = live();
        r.worker.handle(Job::LiveOff);
        r.fake.lock().unwrap().queue(dt1(GAIN, &[50]));
        assert_eq!(r.worker.idle(Instant::now()), vec![]);
    }

    #[test]
    fn live_ends_with_a_disconnect() {
        let mut r = live();
        let t0 = Instant::now();
        r.port.store(false, Ordering::SeqCst);
        assert_eq!(r.worker.idle(t0 + Duration::from_millis(1100)), vec![Event::Disconnected]);
        r.port.store(true, Ordering::SeqCst);
        r.worker.idle(t0 + Duration::from_millis(2200));
        r.fake.lock().unwrap().queue(dt1(GAIN, &[50]));
        assert_eq!(r.worker.idle(t0 + Duration::from_millis(2300)), vec![]);
    }

    #[test]
    fn peek_reads_the_channel_without_switching() {
        let mut r = connected();
        let expected = slot_of(&r.fake, Channel::B3.slot());
        r.fake.lock().unwrap().sent.clear();
        assert_eq!(outcome(r.worker.handle(Job::Peek(Channel::B3))), vec![Event::Peeked(Channel::B3, expected)]);
        assert_eq!(dt1_count(&r.fake), 0);
    }

    fn temp_dir(test: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("katana-gui-worker-{test}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    fn store_job(dir: &std::path::Path) -> Job {
        let mut patch = Patch::default_preset();
        patch.set_name("New");
        Job::Store { ch: Channel::B3, patch, backup_dir: dir.to_path_buf(), stamp: "2026-11-02_20-00-00".into() }
    }

    #[test]
    fn store_writes_backups_first() {
        let dir = temp_dir("backups-first");
        let mut r = connected();
        let old_slot = slot_of(&r.fake, Channel::B3.slot());
        let old_current = current_of(&r.fake);
        let events = outcome(r.worker.handle(store_job(&dir)));
        let [Event::Stored { ch, channel_backup, current_backup, solo_delay_stored }, Event::AmpOn(sel)] =
            events.as_slice()
        else {
            panic!("{events:?}");
        };
        assert_eq!((*ch, *sel, *solo_delay_stored), (Channel::B3, Channel::B3.selector(), false));
        assert_eq!(Patch::from_syx(&std::fs::read(channel_backup).unwrap()).unwrap(), old_slot);
        assert_eq!(Patch::from_syx(&std::fs::read(current_backup).unwrap()).unwrap(), old_current);
        assert!(channel_backup.file_name().unwrap().to_str().unwrap().starts_with("2026-11-02_20-00-00_B3_"));
        assert!(current_backup.file_name().unwrap().to_str().unwrap().starts_with("2026-11-02_20-00-00_current_"));
        assert_eq!(slot_of(&r.fake, Channel::B3.slot()).name(), "New");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn unwritable_backup_stores_nothing() {
        let dir = temp_dir("unwritable");
        std::fs::write(&dir, b"a file, not a folder").unwrap();
        let mut r = connected();
        let before = r.fake.lock().unwrap().mem.clone();
        let events = outcome(r.worker.handle(store_job(&dir)));
        assert!(
            matches!(events.as_slice(), [Event::StoreFailed(m)] if m.contains("Couldn't write the backup") && m.contains("nothing was stored")),
            "{events:?}"
        );
        assert_eq!(dt1_count(&r.fake), 0);
        assert!(r.fake.lock().unwrap().mem == before);
        let _ = std::fs::remove_file(&dir);
    }

    #[test]
    fn store_failure_names_backups() {
        let dir = temp_dir("failure");
        let mut r = connected();
        r.fake.lock().unwrap().ignore_dt1 = true;
        let events = outcome(r.worker.handle(store_job(&dir)));
        let [Event::StoreFailed(m)] = events.as_slice() else { panic!("{events:?}") };
        let files: Vec<_> = std::fs::read_dir(&dir).unwrap().map(|e| e.unwrap().path()).collect();
        assert_eq!(files.len(), 2);
        for f in &files {
            assert!(m.contains(f.to_str().unwrap()), "{m}");
        }
        assert!(m.contains("Load to amp"), "{m}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn unplug_during_store_names_backups_and_disconnects() {
        let dir = temp_dir("unplug");
        let mut r = connected();
        r.fake.lock().unwrap().ignore_dt1 = true;
        r.port.store(false, Ordering::SeqCst);
        let events = outcome(r.worker.handle(store_job(&dir)));
        assert!(
            matches!(events.as_slice(), [Event::StoreFailed(m), Event::Disconnected] if m.contains("Load to amp")),
            "{events:?}"
        );
        assert_eq!(r.worker.handle(Job::LiveOn), vec![Event::Failed("amp not connected".into())]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn live_survives_store() {
        let dir = temp_dir("live");
        let mut r = live();
        r.worker.handle(store_job(&dir));
        assert!(r.fake.lock().unwrap().editor);
        r.fake.lock().unwrap().queue(dt1(GAIN, &[50]));
        assert_eq!(r.worker.idle(Instant::now()), vec![Event::PanelChange { addr: GAIN, data: vec![50] }]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn store_with_live_off_leaves_editor_mode_off() {
        let dir = temp_dir("live-off");
        let mut r = connected();
        r.worker.handle(store_job(&dir));
        assert!(!r.fake.lock().unwrap().editor);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn shadow_follows_the_store() {
        let dir = temp_dir("shadow");
        let mut r = live();
        r.worker.panel_blocks = TEST_PANEL_BLOCKS;
        let t0 = Instant::now();
        r.worker.idle(t0);
        r.worker.handle(store_job(&dir));
        assert_eq!(r.worker.idle(t0 + Duration::from_millis(1100)), vec![]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn shutdown_turns_editor_mode_off() {
        let mut r = live();
        assert_eq!(r.worker.handle(Job::Shutdown), vec![]);
        assert!(!r.fake.lock().unwrap().editor);
    }

    #[test]
    fn dropping_a_connected_worker_turns_editor_mode_off() {
        let r = live();
        let fake = r.fake.clone();
        drop(r.worker);
        assert!(!fake.lock().unwrap().editor);
    }

    #[test]
    fn store_failure_message_says_what_to_do() {
        let m = store_failure_message(
            Channel::A3,
            "boom",
            std::path::Path::new("/b/channel.syx"),
            std::path::Path::new("/b/current.syx"),
        );
        assert_eq!(
            m,
            "Storing into A3 failed: boom. The amp now plays A3 with part of the new preset. \
             Your previous sound: /b/current.syx (Open it, then Load to amp). A3's backup: /b/channel.syx."
        );
    }
}
