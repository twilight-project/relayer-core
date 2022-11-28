use std::collections::HashMap;
use std::sync::mpsc;
use std::sync::Arc;
use std::sync::Mutex;

lazy_static! {
    // pub static ref PENDINGQUEUE: Mutex<HashMap<String, Arc<Mutex<mpsc::Sender<Job>>>>> =
    //     Mutex::new(HashMap::new());
    pub static ref EXECUTORQUEUE: Mutex<HashMap<String, Arc<Mutex<QueueResolverMPSC>>>> =
        Mutex::new(HashMap::new());
}
// pub struct QueueResolverMPSC {
//     sender: Arc<Mutex<mpsc::Sender<Job>>>,
//     receiver: Arc<Mutex<mpsc::Receiver<Job>>>,
// }
pub struct QueueResolverMPSC {
    sender: mpsc::Sender<Job>,
    receiver: Arc<Mutex<mpsc::Receiver<Job>>>,
    pending_task: u32,
}

pub struct QueueResolver {
    // workers: Vec<Worker>,
    sender: mpsc::Sender<Message>,
}

type Job = Box<dyn FnOnce() + Send + 'static>;

enum Message {
    NewJob(Job),
    Terminate,
}

impl QueueResolver {
    pub fn new(resolver_key: String) {
        let (sender, receiver) = mpsc::channel();
        let queue_resolver = QueueResolverMPSC {
            sender,
            receiver: Arc::new(Mutex::new(receiver)),
            pending_task: 0,
        };
        let mut executor_hashmap = EXECUTORQUEUE.lock().unwrap();
        executor_hashmap.insert(
            resolver_key,
            Arc::clone(&Arc::new(Mutex::new(queue_resolver))),
        );
        drop(executor_hashmap);
    }
    // -> QueueResolver {}

    pub fn pending<F>(f: F, resolver_key: String)
    where
        F: FnOnce() + Send + 'static,
    {
        let job = Box::new(f);
        let executor_hashmap = EXECUTORQUEUE.lock().unwrap();
        let executor_queueresolver = executor_hashmap.get(&resolver_key).unwrap().clone();
        drop(executor_hashmap);
        let mut executor_queueresolver_unlock = executor_queueresolver.lock().unwrap();
        executor_queueresolver_unlock.sender.send(job).unwrap();
        executor_queueresolver_unlock.pending_task += 1;
        drop(executor_queueresolver_unlock);
        // pending_task
    }

    pub fn executor(resolver_key: String) {
        let executor_hashmap = EXECUTORQUEUE.lock().unwrap();
        let executor_queueresolver = executor_hashmap.get(&resolver_key).unwrap().clone();
        drop(executor_hashmap);
        let mut executor_queueresolver_unlock = executor_queueresolver.lock().unwrap();
        for i in 0..executor_queueresolver_unlock.pending_task {
            let err = executor_queueresolver_unlock.receiver.lock().unwrap();
            let message = err.recv().unwrap();
            message();
            // println!("awesome success");
        }
        executor_queueresolver_unlock.pending_task = 0;
    }
}
