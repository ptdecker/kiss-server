//! Basic Thread Pool Implementation

use super::*;

use self::worker::Worker;

pub(super) type Job = Box<dyn FnOnce() + Send + 'static>;

#[derive(Debug)]
pub(super) struct ThreadPool {
    workers: Vec<Worker>,
    sender: Option<mpsc::Sender<Job>>,
}

impl ThreadPool {
    /// Creates a new thread pool
    ///
    /// Creates a thread pool of size 'size'.
    ///
    /// # Panics
    ///
    /// `new` will panic if the requested size of the pool is not greater than 1
    ///
    pub(super) fn build(size: usize) -> Result<ThreadPool> {
        if size == 0 {
            return Err(Error::Channel(
                "Cannot create a zero sized thread pool".to_string(),
            ));
        }
        let (sender, receiver) = mpsc::channel();
        let receiver = Arc::new(Mutex::new(receiver));
        let mut workers = Vec::with_capacity(size);
        for id in 0..size {
            workers.push(Worker::new(id, Arc::clone(&receiver))?);
        }
        Ok(ThreadPool {
            workers,
            sender: Some(sender),
        })
    }

    pub(super) fn execute<F>(&self, f: F) -> Result<()>
    where
        F: FnOnce() + Send + 'static,
    {
        let job = Box::new(f);
        self.sender
            .as_ref()
            .ok_or(Error::Channel(
                "expected 'sender' channel was 'None'".to_string(),
            ))?
            .send(job)?;
        Ok(())
    }
}

impl Drop for ThreadPool {
    fn drop(&mut self) {
        drop(self.sender.take());
        for worker in &mut self.workers {
            debug!("Shutting down worker {}", worker.id);
            if let Some(thread) = worker.thread.take() {
                let _ = thread.join();
            }
        }
    }
}
