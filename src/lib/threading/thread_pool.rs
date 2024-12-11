use std::{
    sync::{
        atomic::{AtomicUsize, Ordering},
        mpsc::{self, TryRecvError},
        Arc, Mutex,
    },
    thread,
};

type Job<T> = Box<dyn (FnOnce() -> T) + Send + 'static>;

pub struct ThreadPool<T: Send + 'static> {
    workers: Vec<Worker>,
    sender: Option<mpsc::Sender<Job<T>>>,
    active_jobs: Arc<AtomicUsize>,
    results: Arc<Mutex<Vec<T>>>,
    joined: bool,
}

impl<T: Send + 'static> ThreadPool<T> {
    /// Creates a new thread pool with `size` threads.
    pub fn new(size: usize) -> ThreadPool<T> {
        assert!(size > 0);

        let (sender, receiver) = mpsc::channel();
        let receiver = Arc::new(Mutex::new(receiver));
        let results = Arc::new(Mutex::new(vec![]));

        let active_jobs = Arc::new(AtomicUsize::new(0));

        let mut workers = Vec::with_capacity(size);

        for id in 0..size {
            workers.push(Worker::new(
                id,
                Arc::clone(&receiver),
                Arc::clone(&results),
                Arc::clone(&active_jobs),
            ));
        }

        ThreadPool {
            workers,
            sender: Some(sender),
            active_jobs,
            results,
            joined: false,
        }
    }

    /// Schedule a job to be run on the thread pool.
    pub fn schedule<F>(&self, f: F)
    where
        F: FnOnce() -> T,
        F: Send + 'static,
    {
        self.active_jobs.fetch_add(1, Ordering::SeqCst);

        match &self.sender {
            Some(sender) => sender.send(Box::new(f)).unwrap(),
            _ => panic!("Cannot spawn job after joining the thread pool"),
        }
    }

    /// Gets the results of all finished jobs.
    pub fn get_finished_jobs(&self) -> Vec<T> {
        self.results.lock().unwrap().drain(..).collect()
    }

    /// Join the thread pool, waiting for all jobs to finish.
    /// If `wait_for_scheduled_jobs` is false, the thread pool will cancel scheduled jobs, but will still wait for running jobs to finish.
    /// This is destructive, and the thread pool cannot be used to spawn new jobs after this.
    pub fn join(&mut self, wait_for_scheduled_jobs: bool) {
        if self.joined {
            return;
        }

        self.joined = true;

        drop(self.sender.take().expect("Cannot join thread pool twice"));

        if !wait_for_scheduled_jobs {
            for worker in &mut self.workers {
                worker.send_stop.send(()).unwrap();
            }
        }

        for worker in &mut self.workers {
            // println!("Joining worker {}", worker.id);
            match worker.thread.take().unwrap().join() {
                Ok(_) => (),
                Err(_) => println!("Worker {} failed to join", worker.id),
            };
        }
    }

    pub fn is_joined(&self) -> bool {
        self.joined
    }

    /// Get the number of active jobs.
    pub fn get_active_jobs(&self) -> usize {
        self.active_jobs.load(Ordering::SeqCst)
    }

    /// Blocks the calling thread until there are no active jobs.
    /// Will return immediately if the thread pool has been joined.
    pub fn block_until_no_active_jobs(&self) {
        self.block_until_x_active_jobs(0);
    }

    /// Blocks the calling thread until there are `x` active jobs.
    /// Will return immediately if the thread pool has been joined.
    pub fn block_until_x_active_jobs(&self, x: usize) {
        if self.joined {
            return;
        }

        while self.get_active_jobs() > x {
            thread::sleep(std::time::Duration::from_millis(1));
        }
    }

    /// Blocks the calling thread until there are `x` active jobs, but only if there are more than `y` active jobs.
    /// Will return immediately if the thread pool has been joined.
    pub fn block_until_x_active_jobs_if_above_y(&self, x: usize, y: usize) {
        if self.get_active_jobs() > y {
            self.block_until_x_active_jobs(x);
        }
    }
}

impl<T: Send + 'static> Drop for ThreadPool<T> {
    fn drop(&mut self) {
        self.join(false);
    }
}

struct Worker {
    id: usize,
    thread: Option<thread::JoinHandle<()>>,
    send_stop: mpsc::Sender<()>,
}

impl Worker {
    fn new<T: Send + 'static>(
        id: usize,
        receiver: Arc<Mutex<mpsc::Receiver<Job<T>>>>,
        results: Arc<Mutex<Vec<T>>>,
        active_jobs: Arc<AtomicUsize>,
    ) -> Worker {
        let (send_stop, receive_stop) = mpsc::channel();

        let thread = thread::spawn(move || loop {
            match receive_stop.try_recv() {
                Ok(_) | Err(TryRecvError::Disconnected) => break,
                _ => {}
            }

            let job = match receiver.lock().unwrap().recv() {
                Ok(job) => job,
                Err(_) => break,
            };

            let result = job();

            active_jobs.fetch_sub(1, Ordering::SeqCst);

            results.lock().unwrap().push(result);
        });

        Worker {
            id,
            thread: Some(thread),
            send_stop,
        }
    }
}
