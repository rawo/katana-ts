//! The UI's end of the worker: channels and the thread.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use super::{Event, Job, Worker};

/// How long `shutdown` waits for the worker; a job stuck on the amp must not
/// hold up closing the app.
const SHUTDOWN_WAIT: Duration = Duration::from_secs(2);

pub struct AmpLink {
    jobs: Sender<Job>,
    events: Receiver<Event>,
    thread: Option<JoinHandle<()>>,
    finished: Arc<AtomicBool>,
}

/// Ends the worker from another thread.
pub struct Stopper {
    jobs: Sender<Job>,
    finished: Arc<AtomicBool>,
}

impl Stopper {
    /// Asks the worker to end after the job it is on; true if it did within `limit`.
    pub fn stop(&self, limit: Duration) -> bool {
        let _ = self.jobs.send(Job::Shutdown);
        let deadline = Instant::now() + limit;
        while !self.finished.load(Ordering::SeqCst) {
            if Instant::now() >= deadline {
                return false;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        true
    }
}

/// The worker's side of `AmpLink::pair`, for scripting the app in tests.
pub struct LinkEnd {
    pub jobs: Receiver<Job>,
    pub events: Sender<Event>,
}

impl AmpLink {
    pub fn spawn(worker: Worker, wake: Box<dyn Fn() + Send>) -> AmpLink {
        let (job_tx, job_rx) = channel();
        let (event_tx, event_rx) = channel();
        let finished = Arc::new(AtomicBool::new(false));
        let done = finished.clone();
        let thread = std::thread::spawn(move || {
            // `run` drops the worker when it ends, which turns editor mode off.
            worker.run(job_rx, event_tx, wake);
            done.store(true, Ordering::SeqCst);
        });
        AmpLink { jobs: job_tx, events: event_rx, thread: Some(thread), finished }
    }

    pub fn pair() -> (AmpLink, LinkEnd) {
        let (job_tx, job_rx) = channel();
        let (event_tx, event_rx) = channel();
        (
            AmpLink { jobs: job_tx, events: event_rx, thread: None, finished: Arc::default() },
            LinkEnd { jobs: job_rx, events: event_tx },
        )
    }

    /// Queues a job. A worker that has ended takes nothing, which is fine: it
    /// only ends on shutdown.
    pub fn send(&self, job: Job) {
        let _ = self.jobs.send(job);
    }

    pub fn try_recv(&self) -> Option<Event> {
        self.events.try_recv().ok()
    }

    /// For a signal handler: ends the worker even if the UI doesn't draw frames.
    pub fn stopper(&self) -> Stopper {
        Stopper { jobs: self.jobs.clone(), finished: self.finished.clone() }
    }

    /// Asks the worker to end and waits up to `SHUTDOWN_WAIT` for it.
    pub fn shutdown(self) {
        self.send(Job::Shutdown);
        let Some(thread) = self.thread else { return };
        let deadline = Instant::now() + SHUTDOWN_WAIT;
        while !thread.is_finished() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(5));
        }
        if thread.is_finished() {
            let _ = thread.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::AtomicUsize;

    use super::*;

    #[test]
    fn pair_round_trip() {
        let (link, end) = AmpLink::pair();
        link.send(Job::LiveOn);
        assert_eq!(end.jobs.try_recv().unwrap(), Job::LiveOn);
        end.events.send(Event::Connected).unwrap();
        assert_eq!(link.try_recv(), Some(Event::Connected));
        assert_eq!(link.try_recv(), None);
    }

    fn no_amp() -> Worker {
        Worker::new(Box::new(|| Err(katana_proto::ProtoError::Midi("no amp".into()))), Box::new(|| false))
    }

    #[test]
    fn spawned_worker_answers_and_wakes_the_ui() {
        let wakes = Arc::new(AtomicUsize::new(0));
        let counter = wakes.clone();
        let link = AmpLink::spawn(
            no_amp(),
            Box::new(move || {
                counter.fetch_add(1, Ordering::SeqCst);
            }),
        );
        link.send(Job::ReadSlot(1));
        let deadline = Instant::now() + Duration::from_secs(2);
        let event = loop {
            if let Some(e) = link.try_recv() {
                break e;
            }
            assert!(Instant::now() < deadline, "no event from the worker");
            std::thread::yield_now();
        };
        assert_eq!(event, Event::Failed("amp not connected".into()));
        assert!(wakes.load(Ordering::SeqCst) >= 1);
        link.shutdown();
    }

    #[test]
    fn stopper_ends_the_worker_from_elsewhere() {
        let link = AmpLink::spawn(no_amp(), Box::new(|| {}));
        let stopper = link.stopper();
        assert!(std::thread::spawn(move || stopper.stop(Duration::from_secs(2))).join().unwrap());
    }

    #[test]
    fn stopper_gives_up_at_its_limit() {
        let (link, _end) = AmpLink::pair();
        assert!(!link.stopper().stop(Duration::from_millis(1)));
    }

    #[test]
    fn shutdown_ends_the_worker_thread() {
        let link = AmpLink::spawn(no_amp(), Box::new(|| {}));
        let start = Instant::now();
        link.shutdown();
        assert!(start.elapsed() < Duration::from_secs(1), "shutdown waited for its time limit");
    }
}
