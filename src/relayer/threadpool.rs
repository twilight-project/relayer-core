use crate::error::{RelayerError, Result};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::Duration;
use tracing::{error, info, warn};

type Job = Box<dyn FnOnce() + Send + 'static>;

enum Message {
    NewJob(Job),
    Terminate,
}

pub struct ThreadPool {
    workers: Vec<Worker>,
    sender: mpsc::Sender<Message>,
    name: String,
    max_size: usize,
    current_size: Arc<std::sync::atomic::AtomicUsize>,
}

impl ThreadPool {
    /// Create a new ThreadPool.
    ///
    /// The size is the number of threads in the pool.
    ///
    /// # Panics
    ///
    /// The `new` function will panic if the size is zero.
    pub fn new(size: usize, name: String) -> Result<ThreadPool> {
        if size == 0 {
            return Err(RelayerError::ThreadPool(
                "Thread pool size must be greater than 0".into(),
            ));
        }

        let (sender, receiver) = mpsc::channel();
        let receiver = Arc::new(Mutex::new(receiver));
        let current_size = Arc::new(std::sync::atomic::AtomicUsize::new(0));

        let mut workers = Vec::with_capacity(size);

        for id in 0..size {
            workers.push(Worker::new(
                id,
                Arc::clone(&receiver),
                name.clone(),
                Arc::clone(&current_size),
            ));
        }

        Ok(ThreadPool {
            workers,
            sender,
            name,
            max_size: size,
            current_size,
        })
    }

    pub fn execute<F>(&self, f: F) -> Result<()>
    where
        F: FnOnce() + Send + 'static,
    {
        let job = Box::new(f);

        let current_size = self.current_size.load(std::sync::atomic::Ordering::SeqCst);
        if current_size >= self.max_size {
            warn!(
                "Thread pool {} is at capacity ({}/{})",
                self.name, current_size, self.max_size
            );
        }

        self.sender
            .send(Message::NewJob(job))
            .map_err(|e| RelayerError::ThreadPool(format!("Failed to send job: {}", e)))?;

        Ok(())
    }

    pub fn shutdown(&mut self) {
        println!("Sending terminate message to all workers.");

        for _ in &self.workers {
            self.sender.send(Message::Terminate).unwrap();
        }

        println!("Shutting down all workers.");

        for worker in &mut self.workers {
            println!("Shutting down worker {}", worker.id);

            if let Some(thread) = worker.thread.take() {
                thread.join().unwrap();
            }
        }
    }
}

impl Drop for ThreadPool {
    fn drop(&mut self) {
        info!("Shutting down thread pool: {}", self.name);

        for _ in &self.workers {
            if let Err(e) = self.sender.send(Message::Terminate) {
                error!("Failed to send terminate message: {}", e);
            }
        }

        for worker in &mut self.workers {
            if let Some(thread) = worker.thread.take() {
                if let Err(e) = thread.join() {
                    error!("Failed to join worker thread: {:?}", e);
                }
            }
        }
    }
}

struct Worker {
    id: usize,
    thread: Option<thread::JoinHandle<()>>,
}

impl Worker {
    fn new(
        id: usize,
        receiver: Arc<Mutex<mpsc::Receiver<Message>>>,
        pool_name: String,
        current_size: Arc<std::sync::atomic::AtomicUsize>,
    ) -> Worker {
        let thread = thread::spawn(move || loop {
            let message = {
                let receiver = receiver.lock().unwrap();
                receiver.recv()
            };

            match message {
                Ok(Message::NewJob(job)) => {
                    current_size.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    job();
                    current_size.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
                }
                Ok(Message::Terminate) => {
                    info!("Worker {} in pool {} shutting down", id, pool_name);
                    break;
                }
                Err(e) => {
                    error!(
                        "Worker {} in pool {} encountered error: {}",
                        id, pool_name, e
                    );
                    break;
                }
            }
        });

        Worker {
            id,
            thread: Some(thread),
        }
    }
}
