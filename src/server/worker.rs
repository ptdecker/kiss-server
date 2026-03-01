//! Worker Management

use super::*;

#[derive(Debug)]
pub(super) struct Worker {
    pub(super) id: usize,
    pub(super) thread: Option<thread::JoinHandle<()>>,
}

impl Worker {
    pub(super) fn new(
        id: usize,
        receiver: Arc<Mutex<mpsc::Receiver<pool::Job>>>,
    ) -> Result<Worker> {
        let thread = thread::spawn(move || loop {
            let message = receiver
                .lock()
                .unwrap_or_else(|e| {
                    warn!("Worker {id}: receiver mutex poisoned, recovering");
                    e.into_inner()
                })
                .recv();
            match message {
                Ok(job) => {
                    debug!("Worker {id} got a job; executing.");
                    job();
                }
                Err(_) => {
                    debug!("Worker {id} shutting down.");
                    break;
                }
            }
        });
        Ok(Worker {
            id,
            thread: Some(thread),
        })
    }
}
