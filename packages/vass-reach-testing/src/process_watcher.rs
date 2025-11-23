use std::{
    process::{Child, Command},
    sync::{Arc, Mutex},
    thread::scope,
};
use sysinfo::{MemoryRefreshKind, Pid, RefreshKind, System};

pub struct ProcessWatcher {
    child: Arc<Mutex<Child>>,
    timeout: u64,
}

impl ProcessWatcher {
    pub fn new(child: Arc<Mutex<Child>>, timeout: u64) -> Self {
        Self { child, timeout }
    }

    pub fn watch(&mut self) {
        let mut refresh_kind = RefreshKind::nothing();
        refresh_kind = refresh_kind.with_memory(MemoryRefreshKind::everything());

        let mut sys = System::new_with_specifics(refresh_kind);

        let id = self.child.lock().unwrap().id();
        let pid = Pid::from_u32(id);
        let start = std::time::Instant::now();

        scope(|s| {
            s.spawn(|| {
                loop {
                    // first we check if the process is still running
                    let mut child = self.child.lock().unwrap();
                    let status = child.try_wait();
                    drop(child);

                    match status {
                        Ok(Some(_)) => {
                            // process has exited, we quit watching
                            break;
                        }
                        Ok(None) => {
                            // process is still running
                            std::thread::sleep(std::time::Duration::from_millis(100));
                            // check for timeout and kill if necessary
                            if start.elapsed().as_secs() > self.timeout {
                                println!("Killing process {} for exceeding time limit", pid);
                                let mut child = self.child.lock().unwrap();
                                let _ = child.kill();
                                break;
                            }
                        }
                        Err(_) => {
                            // error checking process status, we quit watching
                            break;
                        }
                    }

                    // get memory usage of the process
                    sys.refresh_memory();

                    if sys.used_memory() > (sys.total_memory() as f64 * 0.8) as u64 {
                        println!("System memory usage exceeded 80%, killing process {}", pid);
                        let mut child = self.child.lock().unwrap();
                        let _ = child.kill();
                        break;
                    }
                }
            });
        });
    }
}

pub fn run_with_watcher(
    command: &mut Command,
    timeout: u64,
) -> Result<std::process::Output, Box<dyn std::error::Error>> {
    let child = command.spawn()?;
    let child_arc = Arc::new(Mutex::new(child));
    let mut watcher = ProcessWatcher::new(Arc::clone(&child_arc), timeout);
    watcher.watch();

    let status = child_arc.lock().unwrap().wait()?;
    if !status.success() {
        return Err(format!("Process terminated with non-ok status {}", status).into());
    }

    let output = command.output()?;

    Ok(output)
}
