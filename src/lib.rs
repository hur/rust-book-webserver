use std::error::Error;
use std::fmt;
use std::thread;
use std::sync::mpsc;
use std::sync::Arc;
use std::sync::Mutex;

pub struct ThreadPool {
    workers: Vec<Worker>,
    sender: mpsc::Sender<Message>,
}

impl ThreadPool {
    /// Create a new ThreadPool
    ///
    /// # Arguments
    ///
    /// size - Number of threads in the pool.
    ///
    /// # Panics
    ///
    /// The function will panic if size is zero.
    pub fn new(size: usize) -> Result<ThreadPool, PoolCreationError> {
        if size == 0 {
            return Err(PoolCreationError::new());
        }
        
        let (sender, receiver) = mpsc::channel();

        // We need to share ownership across multiple threads and allow the threads
        // to mutate the value, hence receiver is Arc<Mutex<mpsc::receiver<Job>>>
        let receiver = Arc::new(Mutex::new(receiver));

        let mut workers = Vec::with_capacity(size);

        for id in 0..size {
            workers.push(Worker::new(id, Arc::clone(&receiver)));
        }
        Ok(ThreadPool{ workers, sender })
    }

	/// Execute a job in the thread pool
	///
	/// # Arguments
	///
	/// f - A closure the pool should run for each stream.
	///
	/// # Panics
	/// 
	/// Panics if receiver goes out of scope i.e. if all the threads
	/// in the pool stop executing.
    pub fn execute<F>(&self, f:F)
    where
        F: FnOnce() + Send + 'static,
    {
        let job = Box::new(f);

        self.sender.send(Message::NewJob(job)).unwrap();
    }
}

impl Drop for ThreadPool {
	/// Drop the thread pool
	/// 
	/// # Panics
	/// 
	/// Panics if the receiver goes out of scope or if thread joining fails.
    fn drop(&mut self) {
        println!("Sending terminate message to all workers");

        for _ in &self.workers {
            self.sender.send(Message::Terminate).unwrap();
        }
        
        for worker in &mut self.workers {
            println!("Shutting down worker {}", worker.id);
            
            if let Some(thread) = worker.thread.take() {
                thread.join().unwrap();
            }
        }
    }
}

type Job = Box<dyn FnOnce() + Send + 'static>;

/// An enum containing the types of messages that Workers understand
enum Message {
	/// Message contains a new job
    NewJob(Job),
	/// Message to terminate
    Terminate,
}

struct Worker {
    id: usize,
    thread: Option<thread::JoinHandle<()>>,
}

impl Worker {
	/// Create a new worker
	/// 
	/// # Arguments
	///
	/// id - id of the worker
	/// receiver - a shared mutable receiver used to receive jobs
	///
	/// # Panics
	///
	/// Panics if mutex is in a poisoned state, or if the sending side of the channel
	/// has shut down 
    fn new(id: usize, receiver: Arc<Mutex<mpsc::Receiver<Message>>>) -> Worker {
        let thread = thread::spawn(move || loop {
            let message = receiver.lock().unwrap().recv().unwrap();
            
            match message {
                Message::NewJob(job) => {
                    println!("Worker {} got a job: executing.", id);
                    job();
                }
                Message::Terminate => {
                    println!("Worker {} was told to terminate", id);
                    break;
                }
            }

        });
        Worker { 
            id,
            thread: Some(thread) ,
        }
    }
}

#[derive(Debug)]
pub struct PoolCreationError {
    details: String,
}

impl PoolCreationError {
    fn new() -> PoolCreationError {
        PoolCreationError{details: String::from("Cannot create a thread pool with 0 threads.")}
    }
}

impl fmt::Display for PoolCreationError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.details)
    }
}

impl Error for PoolCreationError {
    fn description(&self) -> &str {
        &self.details
    }
}

