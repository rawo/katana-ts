//! The UI's end of the worker: channels and the thread.

use std::sync::mpsc::{channel, Receiver, Sender};
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
        let thread = std::thread::spawn(move || worker.run(job_rx, event_tx, wake));
        AmpLink { jobs: job_tx, events: event_rx, thread: Some(thread) }
    }

    pub fn pair() -> (AmpLink, LinkEnd) {
        let (job_tx, job_rx) = channel();
        let (event_tx, event_rx) = channel();
        (AmpLink { jobs: job_tx, events: event_rx, thread: None }, LinkEnd { jobs: job_rx, events: event_tx })
    }

    /// Queues a job. A worker that has ended takes nothing, which is fine: it
    /// only ends on shutdown.
    pub fn send(&self, job: Job) {
        let _ = self.jobs.send(job);
    }

    pub fn try_recv(&self) -> Option<Event> {
        self.events.try_recv().ok()
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
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

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
    fn shutdown_ends_the_worker_thread() {
        let link = AmpLink::spawn(no_amp(), Box::new(|| {}));
        let start = Instant::now();
        link.shutdown();
        assert!(start.elapsed() < Duration::from_secs(1), "shutdown waited for its time limit");
    }
}
