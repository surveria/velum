use core::sync::atomic::{AtomicUsize, Ordering};
use std::thread::{self, JoinHandle};

use tokio::{
    runtime,
    sync::{mpsc, oneshot},
    task::LocalSet,
};
use velum::{Engine, Vm};

use crate::{RuntimeError, VmHandle, actor};

const DEFAULT_VM_COMMAND_CAPACITY: usize = 64;

type VmInitializer = dyn FnOnce(&mut Vm) -> velum::Result<()> + Send;

enum WorkerCommand {
    Spawn {
        engine: Engine,
        initializer: Box<VmInitializer>,
        command_capacity: usize,
        response: oneshot::Sender<Result<VmHandle, RuntimeError>>,
    },
    Shutdown,
}

struct Worker {
    sender: mpsc::UnboundedSender<WorkerCommand>,
    join: Option<JoinHandle<()>>,
}

/// Builder for a pool of single-owner Velum VM workers.
pub struct VmRuntimeBuilder {
    engine: Engine,
    worker_threads: usize,
    command_capacity: usize,
}

impl VmRuntimeBuilder {
    /// Creates a builder using the host's available parallelism.
    #[must_use]
    pub fn new(engine: Engine) -> Self {
        let worker_threads =
            std::thread::available_parallelism().map_or(1, core::num::NonZeroUsize::get);
        Self {
            engine,
            worker_threads,
            command_capacity: DEFAULT_VM_COMMAND_CAPACITY,
        }
    }

    /// Sets the number of Tokio current-thread workers.
    #[must_use]
    pub const fn worker_threads(mut self, worker_threads: usize) -> Self {
        self.worker_threads = worker_threads;
        self
    }

    /// Sets the bounded command capacity allocated for each VM.
    #[must_use]
    pub const fn command_capacity(mut self, command_capacity: usize) -> Self {
        self.command_capacity = command_capacity;
        self
    }

    /// Starts the worker pool.
    ///
    /// # Errors
    /// Fails for zero-sized settings or when a worker cannot be started.
    pub fn build(self) -> Result<VmRuntime, RuntimeError> {
        if self.worker_threads == 0 {
            return Err(RuntimeError::InvalidConfiguration(
                "worker_threads must be greater than zero",
            ));
        }
        if self.command_capacity == 0 {
            return Err(RuntimeError::InvalidConfiguration(
                "command_capacity must be greater than zero",
            ));
        }
        let workers = start_workers(self.worker_threads)?;
        Ok(VmRuntime {
            engine: self.engine,
            workers,
            next_worker: AtomicUsize::new(0),
            command_capacity: self.command_capacity,
        })
    }
}

/// Tokio integration that assigns each VM to exactly one current-thread worker.
///
/// Independent VMs are distributed across worker threads. JavaScript and all
/// VM-local host futures remain on their owning thread, while callers exchange
/// bounded commands through [`VmHandle`].
pub struct VmRuntime {
    engine: Engine,
    workers: Vec<Worker>,
    next_worker: AtomicUsize,
    command_capacity: usize,
}

impl VmRuntime {
    /// Creates a runtime with host-default worker count and queue capacity.
    ///
    /// # Errors
    /// Fails when a worker cannot be started.
    pub fn new(engine: Engine) -> Result<Self, RuntimeError> {
        VmRuntimeBuilder::new(engine).build()
    }

    /// Returns a configurable runtime builder.
    #[must_use]
    pub fn builder(engine: Engine) -> VmRuntimeBuilder {
        VmRuntimeBuilder::new(engine)
    }

    /// Creates a VM without additional initialization.
    ///
    /// # Errors
    /// Fails when the runtime is closed or the worker cannot create the VM.
    pub async fn spawn_vm(&self) -> Result<VmHandle, RuntimeError> {
        self.spawn_vm_with(|_vm| Ok(())).await
    }

    /// Creates and initializes a VM on its permanent owning thread.
    ///
    /// The initializer is the right place to register host functions and host
    /// classes. It runs once before the handle becomes visible.
    ///
    /// # Errors
    /// Fails when initialization fails or the selected worker is closed.
    pub async fn spawn_vm_with<F>(&self, initializer: F) -> Result<VmHandle, RuntimeError>
    where
        F: FnOnce(&mut Vm) -> velum::Result<()> + Send + 'static,
    {
        let Some(worker) = self.select_worker() else {
            return Err(RuntimeError::RuntimeClosed);
        };
        let (response, receiver) = oneshot::channel();
        worker
            .sender
            .send(WorkerCommand::Spawn {
                engine: self.engine.clone(),
                initializer: Box::new(initializer),
                command_capacity: self.command_capacity,
                response,
            })
            .map_err(|_error| RuntimeError::RuntimeClosed)?;
        receiver
            .await
            .map_err(|_error| RuntimeError::ResponseDropped)?
    }

    fn select_worker(&self) -> Option<&Worker> {
        let count = self.workers.len();
        if count == 0 {
            return None;
        }
        let previous = self
            .next_worker
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
                Some(value.overflowing_add(1).0)
            })
            .unwrap_or_else(|value| value);
        self.workers.get(previous % count)
    }
}

impl Drop for VmRuntime {
    fn drop(&mut self) {
        stop_workers(&mut self.workers);
    }
}

fn start_workers(count: usize) -> Result<Vec<Worker>, RuntimeError> {
    let mut workers = Vec::new();
    workers
        .try_reserve_exact(count)
        .map_err(|error| RuntimeError::WorkerStart(error.to_string()))?;
    for index in 0..count {
        let tokio_runtime = runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|error| {
                stop_workers(&mut workers);
                RuntimeError::WorkerStart(error.to_string())
            })?;
        let (sender, receiver) = mpsc::unbounded_channel();
        let name = format!("velum-vm-worker-{index}");
        let join = thread::Builder::new()
            .name(name)
            .spawn(move || {
                let local_set = LocalSet::new();
                local_set.block_on(&tokio_runtime, worker_loop(receiver));
            })
            .map_err(|error| {
                stop_workers(&mut workers);
                RuntimeError::WorkerStart(error.to_string())
            })?;
        workers.push(Worker {
            sender,
            join: Some(join),
        });
    }
    Ok(workers)
}

async fn worker_loop(mut commands: mpsc::UnboundedReceiver<WorkerCommand>) {
    while let Some(command) = commands.recv().await {
        match command {
            WorkerCommand::Spawn {
                engine,
                initializer,
                command_capacity,
                response,
            } => {
                spawn_vm_actor(&engine, initializer, command_capacity, response);
            }
            WorkerCommand::Shutdown => return,
        }
    }
}

fn spawn_vm_actor(
    engine: &Engine,
    initializer: Box<VmInitializer>,
    command_capacity: usize,
    response: oneshot::Sender<Result<VmHandle, RuntimeError>>,
) {
    let mut vm = engine.create_vm();
    if let Err(error) = initializer(&mut vm) {
        drop(response.send(Err(RuntimeError::engine(&error))));
        return;
    }
    let (sender, receiver) = mpsc::channel(command_capacity);
    let task = tokio::task::spawn_local(actor::run(vm, receiver));
    drop(task);
    drop(response.send(Ok(VmHandle { sender })));
}

fn stop_workers(workers: &mut [Worker]) {
    for worker in &*workers {
        drop(worker.sender.send(WorkerCommand::Shutdown));
    }
    let current_thread = thread::current().id();
    for worker in workers {
        if let Some(join) = worker.join.take() {
            if join.thread().id() == current_thread {
                drop(join);
            } else {
                drop(join.join());
            }
        }
    }
}
