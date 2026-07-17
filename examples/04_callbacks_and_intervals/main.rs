use std::{
    cell::RefCell,
    future::Future,
    rc::Rc,
    task::{Context as TaskContext, Poll, Waker},
};

use velum::{
    Engine, Error, HostFutureError, JsValueRef, OwnedValue, PropertyKeyRef, QueuedCallRequest,
    QueuedCallResult, RetainedValue, Vm,
};

const TIMER_ID: u32 = 1;
const TICK_COUNT: u32 = 10;

#[derive(Default)]
struct VirtualInterval {
    callback: Option<RetainedValue>,
    elapsed_ticks: u32,
}

impl VirtualInterval {
    fn install(&mut self, callback: RetainedValue) -> Result<u32, Error> {
        if self.callback.is_some() {
            return Err(Error::runtime("this example supports one interval"));
        }
        self.callback = Some(callback);
        self.elapsed_ticks = 0;
        Ok(TIMER_ID)
    }

    fn advance(&mut self) -> Result<(), Error> {
        self.elapsed_ticks = self
            .elapsed_ticks
            .checked_add(1)
            .ok_or_else(|| Error::runtime("virtual timer tick overflowed"))?;
        Ok(())
    }

    const fn callback(&self) -> Option<&RetainedValue> {
        self.callback.as_ref()
    }

    fn clear(&mut self, id: u32) -> Result<bool, Error> {
        if id != TIMER_ID {
            return Ok(false);
        }
        let Some(callback) = self.callback.take() else {
            return Ok(false);
        };
        callback.release()?;
        Ok(true)
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut vm = Engine::new().create_vm();
    let timer = Rc::new(RefCell::new(VirtualInterval::default()));
    install_timer_api(&mut vm, &timer)?;

    vm.eval(
        r"
        class CallbackTicker {
            constructor(callback) {
                this.callback = callback;
                this.count = 0;
            }

            start() {
                this.timer = setInterval(() => {
                    this.count += 1;
                    this.callback(this.count);
                    if (this.count === 10) clearInterval(this.timer);
                }, 1);
            }
        }
        ",
    )?;

    let calls = Rc::new(RefCell::new(Vec::new()));
    let captured_calls = Rc::clone(&calls);
    let rust_callback = vm.create_host_function_typed("rustTick", move |call| {
        let tick = call.number(0, "tick")?;
        captured_calls.borrow_mut().push(tick);
        println!("Rust callback tick {tick}");
        Ok(())
    })?;
    let constructor = vm
        .get_global_retained("CallbackTicker")?
        .ok_or("CallbackTicker was not defined")?;
    let ticker = vm.construct_retained(&constructor, &[JsValueRef::Retained(&rust_callback)])?;
    constructor.release()?;
    rust_callback.release()?;
    vm.call_method_owned((&ticker).into(), PropertyKeyRef::Name("start"), &[])?;

    for _ in 0..TICK_COUNT {
        timer.borrow_mut().advance()?;
        let request = {
            let timer = timer.borrow();
            let callback = timer.callback().ok_or("interval ended too early")?;
            vm.enqueue_call(callback, &[])?
        };
        vm.run_host_commands()?;
        vm.run_jobs()?;
        let result = ready_request(request)?;
        if !matches!(result, QueuedCallResult::Owned(OwnedValue::Undefined)) {
            return Err(format!("unexpected interval result: {result:?}").into());
        }
    }

    if timer.borrow().callback().is_some() {
        return Err("clearInterval did not release the scheduled callback".into());
    }
    let expected = (1..=TICK_COUNT).map(f64::from).collect::<Vec<_>>();
    if calls.borrow().as_slice() != expected.as_slice() {
        return Err(format!("expected {expected:?}, got {:?}", calls.borrow()).into());
    }
    ticker.release()?;
    vm.collect_garbage()?;
    Ok(())
}

fn install_timer_api(vm: &mut Vm, timer: &Rc<RefCell<VirtualInterval>>) -> Result<(), Error> {
    let set_timer = Rc::clone(timer);
    vm.register_host_function_typed("setInterval", move |call| {
        let callback = call.required_value(0, "callback")?.retain()?;
        let delay = call.number(1, "delay")?;
        if (delay - 1.0).abs() > f64::EPSILON {
            return Err(Error::runtime(
                "this virtual clock expects a one-tick delay",
            ));
        }
        set_timer.borrow_mut().install(callback).map(f64::from)
    })?;

    let clear_timer = Rc::clone(timer);
    vm.register_host_function_typed("clearInterval", move |call| {
        let id = call.number(0, "id")?;
        if (id - f64::from(TIMER_ID)).abs() > f64::EPSILON {
            return Ok(false);
        }
        clear_timer.borrow_mut().clear(TIMER_ID)
    })
}

fn ready_request(
    request: QueuedCallRequest,
) -> Result<QueuedCallResult, Box<dyn std::error::Error>> {
    let mut request = Box::pin(request);
    let mut context = TaskContext::from_waker(Waker::noop());
    match request.as_mut().poll(&mut context) {
        Poll::Ready(result) => result.map_err(host_error),
        Poll::Pending => Err("interval callback did not settle synchronously".into()),
    }
}

fn host_error(error: HostFutureError) -> Box<dyn std::error::Error> {
    Box::new(error)
}
