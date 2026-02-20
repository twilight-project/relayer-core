use std::sync::mpsc;
use std::sync::Arc;
use std::sync::Mutex;
use std::thread;

pub struct ThreadPool {
    name: String,
    workers: Vec<Worker>,
    sender: mpsc::Sender<Message>,
}

type Job = Box<dyn FnOnce() + Send + 'static>;

enum Message {
    NewJob(Job),
    Terminate,
}

impl ThreadPool {
    /// Create a new ThreadPool.
    ///
    /// The size is the number of threads in the pool.
    ///
    /// # Panics
    ///
    /// The `new` function will panic if the size is zero.
    pub fn new(size: usize, t_name: String) -> ThreadPool {
        assert!(size > 0);

        let (sender, receiver) = mpsc::channel();

        let receiver = Arc::new(Mutex::new(receiver));

        let mut workers = Vec::with_capacity(size);

        for id in 0..size {
            workers.push(Worker::new(id, t_name.clone(), Arc::clone(&receiver)));
        }

        ThreadPool { name: t_name, workers, sender }
    }

    pub fn execute<F>(&self, f: F)
    where
        F: FnOnce() + Send + 'static,
    {
        let job = Box::new(f);

        if let Err(e) = self.sender.send(Message::NewJob(job)) {
            crate::log_heartbeat!(error, "ThreadPool [{}]: failed to send job: {:?}", self.name, e);
        }
    }

    pub fn shutdown(&mut self) {
        crate::log_heartbeat!(warn, "ThreadPool [{}]: Sending terminate message to all workers.", self.name);

        for _ in &self.workers {
            if let Err(e) = self.sender.send(Message::Terminate) {
                crate::log_heartbeat!(error, "ThreadPool [{}]: failed to send terminate: {:?}", self.name, e);
            }
        }

        crate::log_heartbeat!(warn, "ThreadPool [{}]: Shutting down all workers.", self.name);

        for worker in &mut self.workers {
            crate::log_heartbeat!(warn, "ThreadPool [{}]: Shutting down worker {}", self.name, worker.id);

            if let Some(thread) = worker.thread.take() {
                if let Err(e) = thread.join() {
                    crate::log_heartbeat!(error, "ThreadPool [{}]: worker {} thread panicked: {:?}", self.name, worker.id, e);
                }
            }
        }
    }
}

impl Drop for ThreadPool {
    fn drop(&mut self) {
        crate::log_heartbeat!(warn, "ThreadPool [{}]: Sending terminate message to all workers.", self.name);

        for _ in &self.workers {
            if let Err(e) = self.sender.send(Message::Terminate) {
                crate::log_heartbeat!(error, "ThreadPool [{}]: failed to send terminate: {:?}", self.name, e);
            }
        }

        crate::log_heartbeat!(warn, "ThreadPool [{}]: Shutting down all workers.", self.name);

        for worker in &mut self.workers {
            crate::log_heartbeat!(warn, "ThreadPool [{}]: Shutting down worker {}", self.name, worker.id);

            if let Some(thread) = worker.thread.take() {
                if let Err(e) = thread.join() {
                    crate::log_heartbeat!(error, "ThreadPool [{}]: worker {} thread panicked: {:?}", self.name, worker.id, e);
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
    fn new(id: usize, t_name: String, receiver: Arc<Mutex<mpsc::Receiver<Message>>>) -> Worker {
        let thread_name = format!("{}-{}", t_name, id);
        let thread_name_clone = thread_name.clone();
        let thread = thread::Builder::new()
            .name(thread_name.clone())
            .spawn(move || loop {
                let message = match receiver.lock() {
                    Ok(rx) => match rx.recv() {
                        Ok(msg) => msg,
                        Err(_) => break, // channel closed, exit worker
                    },
                    Err(e) => {
                        crate::log_heartbeat!(error, "ThreadPool worker [{}]: mutex poisoned: {:?}", thread_name, e);
                        break;
                    }
                };

                match message {
                    Message::NewJob(job) => {
                        job();
                    }
                    Message::Terminate => {
                        break;
                    }
                }
            });

        Worker {
            id,
            thread: match thread {
                Ok(t) => Some(t),
                Err(e) => {
                    crate::log_heartbeat!(error, "ThreadPool: failed to spawn worker thread [{}]: {:?}", thread_name_clone, e);
                    None
                }
            },
        }
    }
}
