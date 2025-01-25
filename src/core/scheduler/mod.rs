use std::{
    cmp::Reverse,
    collections::BinaryHeap,
    time::{Duration, Instant},
};

use anyhow::Result;
use crossbeam::channel::{Receiver, Sender, after, never, select_biased, unbounded};

use self::{job::Job, repeated_job::RepeatedJob};
use crate::shared::{
    id::{self, Id},
    macros::{try_cont, try_skip},
};

mod job;
mod repeated_job;

/// Scheduler can run jobs after a specified duration or at a specified
/// interval. These jobs run on a separate thread and are guaranteed to run
/// after at least the specified time. Scheduled jobs must never block the
/// thread as that will stop other jobs from running. Stopping the scheduler
/// will block until the current job is finished and all other jobs in the queue
/// are discarded.
#[derive(Debug)]
pub(crate) struct Scheduler<T: Clone + Send + 'static + std::fmt::Debug> {
    add_job_tx: Sender<SchedulerCommand<T>>,
    add_job_rx: Receiver<SchedulerCommand<T>>,
    args: T,
    handle: Option<std::thread::JoinHandle<()>>,
}

impl<T: Clone + Send + 'static + std::fmt::Debug> Scheduler<T> {
    /// Create a new scheduler. The [`args`] will be available to all the jobs
    /// as a parameter by reference. Args must be [`Clone`] because it is cloned
    /// every time the scheduler is started in order to move it to the jobs
    /// thread.
    pub(crate) fn new(args: T) -> Self {
        let (add_job_tx, add_job_rx) = unbounded::<SchedulerCommand<T>>();
        Self { add_job_tx, add_job_rx, args, handle: None }
    }

    /// Starts the scheduler if not running already.
    pub(crate) fn start(&mut self) {
        if self.handle.is_some() {
            return;
        }

        let add_job_rx = self.add_job_rx.clone();
        let args = self.args.clone();

        self.handle = Some(std::thread::spawn(move || {
            let mut duration = None;
            let mut jobs = BinaryHeap::new();
            loop {
                let timeout = duration.map_or_else(never, after);
                log::trace!(duration:?; "Starting new schedule loop");
                // Bias towards the job receiver so we do not lose jobs when timeout happens at
                // the same time
                select_biased!(
                    recv(add_job_rx) -> job => {
                        let job = try_cont!(job, "Failed to process scheduler command");
                        match job {
                            SchedulerCommand::AddJob(job) => jobs.push(Reverse(JobOrRepeatedJob::Job(job))),
                            SchedulerCommand::AddRepeatedJob(job) => jobs.push(Reverse(JobOrRepeatedJob::RepeatedJob(job))),
                            SchedulerCommand::CancelJob(id) => jobs.retain(|job| match &job.0 {
                                    JobOrRepeatedJob::Job(job) => job.id != id,
                                    JobOrRepeatedJob::RepeatedJob(job) => job.id != id,
                                }
                            ),
                            SchedulerCommand::StopScheduler => break,
                        }
                    }
                    recv(timeout) -> _ => log::trace!(jobs:?; "Scheduler timed out, trying to run a job"),
                );

                let now = Instant::now();
                match jobs.peek() {
                    Some(Reverse(job)) if job.run_at() <= now => match jobs.pop() {
                        Some(Reverse(JobOrRepeatedJob::Job(job))) => job.run(&args),
                        Some(Reverse(JobOrRepeatedJob::RepeatedJob(mut job))) => {
                            job.run(&args, now);
                            // Add the job back to the queue after it has been ran and its next
                            // run_at has been calculated
                            jobs.push(Reverse(JobOrRepeatedJob::RepeatedJob(job)));
                        }
                        _ => {}
                    },
                    _ => {}
                };

                duration =
                    jobs.peek().map(|Reverse(job)| job.run_at().saturating_duration_since(now));
                log::trace!(duration:?, jobs:? = jobs; "Schedule loop finished, waiting");
            }
        }));
    }

    /// Stops the scheduler. Unprocessed jobs still in the queue are discarded.
    /// Job that is currently running will run to the end and will block the
    /// current thread.
    pub(crate) fn stop(&mut self) {
        if let Some(handle) = self.handle.take() {
            self.add_job_tx.send(SchedulerCommand::StopScheduler).expect("");
            handle.join().expect("");
        }
    }

    /// Schedules a job to run after the specified duration.
    /// A job must guarantee that it will not block the scheduler.
    pub(crate) fn schedule(
        &mut self,
        timeout: Duration,
        callback: impl FnOnce(&T) -> Result<()> + Send + 'static,
    ) {
        let id = id::new();
        // Skip errors as this should never really happen, but still want to log it in
        // case it does
        try_skip!(
            self.add_job_tx.send(SchedulerCommand::AddJob(Job::new(id, timeout, callback))),
            "Failed to schedule a job"
        );
    }

    /// Schedule a repeated job to run at the specified interval.
    /// First run will happen after the specified interval.
    /// A job must guarantee that it will not block the scheduler.
    /// Returns a guard that cancels the job when dropped. Caencellation does
    /// not guarantee that the job will not run at least once more.
    #[must_use = "When the return value is dropped the job is cancelled"]
    pub(crate) fn repeated(
        &mut self,
        interval: Duration,
        callback: impl FnMut(&T) -> Result<()> + Send + 'static,
    ) -> TaskGuard<T> {
        let id = id::new();
        // Skip errors as this should never really happen, but still want to log it in
        // case it does
        try_skip!(
            self.add_job_tx
                .send(SchedulerCommand::AddRepeatedJob(RepeatedJob::new(id, interval, callback))),
            "Failed to schedule a repeated job"
        );

        TaskGuard { id, job_tx: self.add_job_tx.clone() }
    }
}

impl<T: Clone + Send + 'static + std::fmt::Debug> Drop for Scheduler<T> {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Cancels the taks when dropped
pub(crate) struct TaskGuard<T> {
    id: Id,
    job_tx: Sender<SchedulerCommand<T>>,
}

impl<T> Drop for TaskGuard<T> {
    fn drop(&mut self) {
        try_skip!(self.job_tx.send(SchedulerCommand::CancelJob(self.id)), "Failed to cancel job");
    }
}

#[derive(Debug)]
enum SchedulerCommand<T> {
    AddJob(Job<T>),
    AddRepeatedJob(RepeatedJob<T>),
    CancelJob(Id),
    StopScheduler,
}

#[derive(Debug)]
enum JobOrRepeatedJob<T> {
    Job(Job<T>),
    RepeatedJob(RepeatedJob<T>),
}

impl<T> JobOrRepeatedJob<T> {
    fn run_at(&self) -> Instant {
        match self {
            JobOrRepeatedJob::Job(job) => job.run_at,
            JobOrRepeatedJob::RepeatedJob(job) => job.run_at,
        }
    }
}

impl<T> PartialEq for JobOrRepeatedJob<T> {
    fn eq(&self, other: &Self) -> bool {
        self.run_at() == other.run_at()
    }
}
impl<T> Eq for JobOrRepeatedJob<T> {}

impl<T> PartialOrd for JobOrRepeatedJob<T> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl<T> Ord for JobOrRepeatedJob<T> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.run_at().cmp(&other.run_at())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use std::{
        sync::{Arc, Mutex},
        time::Duration,
    };

    use super::Scheduler;

    #[test]
    fn schedules_jobs_in_the_correct_order() {
        let mut scheduler = Scheduler::new(());
        let results = Arc::new(Mutex::new(Vec::new()));

        let res = Arc::clone(&results);
        scheduler.schedule(Duration::from_millis(40), move |()| {
            let mut results = res.lock().unwrap();
            if results.len() < 4 {
                results.push(4);
            }
            Ok(())
        });
        let res = Arc::clone(&results);
        scheduler.schedule(Duration::from_millis(10), move |()| {
            let mut results = res.lock().unwrap();
            if results.len() < 4 {
                results.push(3);
            }
            Ok(())
        });
        let res = Arc::clone(&results);
        scheduler.schedule(Duration::from_millis(20), move |()| {
            let mut results = res.lock().unwrap();
            if results.len() < 4 {
                results.push(2);
            }
            Ok(())
        });
        let res = Arc::clone(&results);
        scheduler.schedule(Duration::from_millis(30), move |()| {
            let mut results = res.lock().unwrap();
            if results.len() < 4 {
                results.push(1);
            }
            Ok(())
        });

        scheduler.start();
        while results.lock().unwrap().len() < 4 {
            std::thread::sleep(Duration::from_millis(10));
        }
        scheduler.stop();

        assert_eq!(*results.lock().unwrap(), vec![3, 2, 1, 4]);
    }

    #[test]
    fn schedules_repeated_jobs() {
        let mut scheduler = Scheduler::new(());
        let results = Arc::new(Mutex::new(Vec::new()));

        let res = Arc::clone(&results);
        let guard1 = scheduler.repeated(Duration::from_millis(10), move |()| {
            let mut results = res.lock().unwrap();
            if results.len() < 6 {
                results.push(1);
            }
            Ok(())
        });
        let res = Arc::clone(&results);
        let guard2 = scheduler.repeated(Duration::from_millis(10), move |()| {
            let mut results = res.lock().unwrap();
            if results.len() < 6 {
                results.push(2);
            }
            Ok(())
        });

        scheduler.start();
        while results.lock().unwrap().len() < 6 {
            std::thread::sleep(Duration::from_millis(10));
        }
        drop(guard1);
        drop(guard2);
        scheduler.stop();

        assert_eq!(*results.lock().unwrap(), vec![1, 2, 1, 2, 1, 2]);
    }

    #[test]
    fn interleaves_repeated_and_scheduled_jobs() {
        let expected_results = 9;
        let mut scheduler = Scheduler::new(());
        let results = Arc::new(Mutex::new(Vec::new()));

        let res = Arc::clone(&results);
        scheduler.schedule(Duration::from_millis(5), move |()| {
            let mut results = res.lock().unwrap();
            if results.len() < expected_results {
                results.push(5);
            }
            Ok(())
        });
        let res = Arc::clone(&results);
        scheduler.schedule(Duration::from_millis(8), move |()| {
            let mut results = res.lock().unwrap();
            if results.len() < expected_results {
                results.push(6);
            }
            Ok(())
        });
        let res = Arc::clone(&results);
        scheduler.schedule(Duration::from_millis(15), move |()| {
            let mut results = res.lock().unwrap();
            if results.len() < expected_results {
                results.push(7);
            }
            Ok(())
        });
        let res = Arc::clone(&results);
        scheduler.schedule(Duration::from_millis(18), move |()| {
            let mut results = res.lock().unwrap();
            if results.len() < expected_results {
                results.push(8);
            }
            Ok(())
        });
        let res = Arc::clone(&results);
        scheduler.schedule(Duration::from_millis(25), move |()| {
            let mut results = res.lock().unwrap();
            if results.len() < expected_results {
                results.push(9);
            }
            Ok(())
        });
        let res = Arc::clone(&results);
        let guard1 = scheduler.repeated(Duration::from_millis(10), move |()| {
            let mut results = res.lock().unwrap();
            if results.len() < expected_results {
                results.push(1);
            }
            Ok(())
        });
        let res = Arc::clone(&results);
        let guard2 = scheduler.repeated(Duration::from_millis(10), move |()| {
            let mut results = res.lock().unwrap();
            if results.len() < expected_results {
                results.push(2);
            }
            Ok(())
        });

        scheduler.start();
        while results.lock().unwrap().len() < expected_results {
            std::thread::sleep(Duration::from_millis(10));
        }
        drop(guard1);
        drop(guard2);
        scheduler.stop();

        assert_eq!(*results.lock().unwrap(), vec![5, 6, 1, 2, 7, 8, 1, 2, 9]);
    }
}
