use core::{future::poll_fn, task::Poll};

use tokio::sync::{mpsc, oneshot};
use velum::Vm;

use crate::{RuntimeError, command::VmCommand};

// The VM is deliberately !Send and this actor is spawned only on a LocalSet.
#[allow(clippy::future_not_send)]
pub async fn run(mut vm: Vm, mut commands: mpsc::Receiver<VmCommand>) {
    let mut fault = None;
    let mut idle_waiters = Vec::new();

    loop {
        notify_if_idle(&vm, fault.as_ref(), &mut idle_waiters);
        if fault.is_some() {
            let Some(command) = commands.recv().await else {
                reject_waiters(&mut idle_waiters, &RuntimeError::VmClosed);
                return;
            };
            reject_command(command, fault.as_ref());
            continue;
        }

        if has_pending_work(&vm) {
            tokio::select! {
                biased;
                command = commands.recv() => {
                    let Some(command) = command else {
                        reject_waiters(&mut idle_waiters, &RuntimeError::VmClosed);
                        return;
                    };
                    handle_command(command, &mut vm, &mut fault, &mut idle_waiters);
                }
                result = poll_activity(&mut vm) => {
                    if let Err(error) = result {
                        record_fault(&error, &mut fault, &mut idle_waiters);
                    }
                }
            }
        } else {
            let Some(command) = commands.recv().await else {
                reject_waiters(&mut idle_waiters, &RuntimeError::VmClosed);
                return;
            };
            handle_command(command, &mut vm, &mut fault, &mut idle_waiters);
        }
    }
}

fn handle_command(
    command: VmCommand,
    vm: &mut Vm,
    fault: &mut Option<RuntimeError>,
    idle_waiters: &mut Vec<oneshot::Sender<Result<(), RuntimeError>>>,
) {
    match command {
        VmCommand::Run(operation) => {
            operation.execute(vm);
            if let Err(error) = run_ready_work(vm) {
                record_fault(&error, fault, idle_waiters);
            }
        }
        VmCommand::RunLocal(operation) => {
            operation.start(vm);
            if let Err(error) = run_ready_work(vm) {
                record_fault(&error, fault, idle_waiters);
            }
        }
        VmCommand::WaitIdle(response) => {
            if is_idle(vm) {
                drop(response.send(Ok(())));
            } else {
                idle_waiters.push(response);
            }
        }
    }
}

fn reject_command(command: VmCommand, fault: Option<&RuntimeError>) {
    let error = fault.cloned().unwrap_or(RuntimeError::VmClosed);
    match command {
        VmCommand::Run(operation) => operation.reject(error),
        VmCommand::RunLocal(operation) => operation.reject(error),
        VmCommand::WaitIdle(response) => drop(response.send(Err(error))),
    }
}

// VM-local host futures may also be !Send and use the same LocalSet.
#[allow(clippy::future_not_send)]
async fn poll_activity(vm: &mut Vm) -> Result<(), RuntimeError> {
    poll_fn(|task_context| {
        let polled = match vm.poll_host_futures(task_context) {
            Ok(polled) => polled,
            Err(error) => return Poll::Ready(Err(RuntimeError::engine(&error))),
        };
        let host_commands = match vm.run_host_commands() {
            Ok(count) => count,
            Err(error) => return Poll::Ready(Err(RuntimeError::engine(&error))),
        };
        let jobs = match vm.run_jobs() {
            Ok(count) => count,
            Err(error) => return Poll::Ready(Err(RuntimeError::engine(&error))),
        };
        if polled.completed() > 0 || host_commands > 0 || jobs > 0 {
            Poll::Ready(Ok(()))
        } else {
            Poll::Pending
        }
    })
    .await
}

fn run_ready_work(vm: &mut Vm) -> Result<(), RuntimeError> {
    vm.run_host_commands()
        .map_err(|error| RuntimeError::engine(&error))?;
    vm.run_jobs()
        .map_err(|error| RuntimeError::engine(&error))?;
    Ok(())
}

fn has_pending_work(vm: &Vm) -> bool {
    vm.pending_host_future_count() > 0
        || vm.pending_host_command_count() > 0
        || vm.pending_job_count() > 0
}

fn is_idle(vm: &Vm) -> bool {
    !has_pending_work(vm)
}

fn notify_if_idle(
    vm: &Vm,
    fault: Option<&RuntimeError>,
    waiters: &mut Vec<oneshot::Sender<Result<(), RuntimeError>>>,
) {
    if fault.is_none() && is_idle(vm) {
        for waiter in core::mem::take(waiters) {
            drop(waiter.send(Ok(())));
        }
    }
}

fn record_fault(
    error: &RuntimeError,
    fault: &mut Option<RuntimeError>,
    waiters: &mut Vec<oneshot::Sender<Result<(), RuntimeError>>>,
) {
    if fault.is_none() {
        *fault = Some(error.clone());
    }
    reject_waiters(waiters, error);
}

fn reject_waiters(
    waiters: &mut Vec<oneshot::Sender<Result<(), RuntimeError>>>,
    error: &RuntimeError,
) {
    for waiter in core::mem::take(waiters) {
        drop(waiter.send(Err(error.clone())));
    }
}
