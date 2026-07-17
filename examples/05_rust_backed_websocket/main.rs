use std::{mem::size_of, sync::Arc};

use parking_lot::Mutex;
use velum::{Engine, Error, HostClass, HostInstance, HostMethodResult, OwnedValue};

const CONNECTING: u8 = 0;
const OPEN: u8 = 1;
const CLOSED: u8 = 3;
const MAX_MESSAGES: usize = 8;
const MAX_MESSAGE_BYTES: usize = 256;
const LOGICAL_PAYLOAD_BYTES: usize =
    size_of::<MockSocket>().saturating_add(MAX_MESSAGES.saturating_mul(MAX_MESSAGE_BYTES));

#[derive(Debug)]
struct MockSocket {
    url: String,
    ready_state: u8,
    buffered_amount: usize,
    sent_messages: Vec<String>,
}

type SharedSocket = Arc<Mutex<MockSocket>>;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut vm = Engine::new().create_vm();
    vm.register_host_class(websocket_class())?;

    let result = vm.eval_owned(
        r#"
        const socket = new WebSocket("wss://example.invalid/events");
        socket.send("first");
        const clone = socket.cloneHandle();
        clone.send("second");
        const summary = [
            socket.url,
            socket.readyState,
            socket.sentMessages(),
            socket === clone
        ].join(" | ");
        clone.close();
        summary + " | closed=" + socket.readyState;
        "#,
    )?;
    let OwnedValue::String(summary) = result else {
        return Err("mock WebSocket did not return a summary string".into());
    };
    println!("{summary}");
    Ok(())
}

fn websocket_class() -> HostClass<SharedSocket> {
    HostClass::new("WebSocket", |call| {
        let url = call.string(0, "url")?.to_owned();
        let state = MockSocket {
            url,
            ready_state: OPEN,
            buffered_amount: 0,
            sent_messages: Vec::with_capacity(MAX_MESSAGES),
        };
        Ok(HostInstance::new(
            Arc::new(Mutex::new(state)),
            LOGICAL_PAYLOAD_BYTES,
        ))
    })
    .with_constructor_length(1)
    .getter("url", |socket, _call| Ok(socket.lock().url.clone()))
    .getter("readyState", |socket, _call| {
        Ok(f64::from(socket.lock().ready_state))
    })
    .getter("bufferedAmount", |socket, _call| {
        usize_to_number(socket.lock().buffered_amount)
    })
    .method_with_length("send", 1, |socket, call| {
        let message = call.string(0, "message")?;
        let mut state = socket.lock();
        if state.ready_state != OPEN {
            return Err(Error::runtime("mock WebSocket is not open"));
        }
        if message.len() > MAX_MESSAGE_BYTES {
            return Err(Error::runtime("mock WebSocket message is too large"));
        }
        if state.sent_messages.len() >= MAX_MESSAGES {
            return Err(Error::runtime("mock WebSocket history is full"));
        }
        state.buffered_amount = state
            .buffered_amount
            .checked_add(message.len())
            .ok_or_else(|| Error::runtime("mock buffered amount overflowed"))?;
        state.sent_messages.push(message.to_owned());
        state.buffered_amount = 0;
        drop(state);
        Ok(())
    })
    .method("close", |socket, _call| {
        socket.lock().ready_state = CLOSED;
        Ok(())
    })
    .method("sentMessages", |socket, _call| {
        Ok(socket.lock().sent_messages.join(","))
    })
    .method_with_result("cloneHandle", 0, |_socket, _call| {
        Ok(HostMethodResult::shared_receiver())
    })
    .static_method("stateName", 1, |call| {
        let state = call.number(0, "state")?;
        let name = if same_number(state, CONNECTING) {
            "CONNECTING"
        } else if same_number(state, OPEN) {
            "OPEN"
        } else if same_number(state, CLOSED) {
            "CLOSED"
        } else {
            "UNKNOWN"
        };
        Ok(name)
    })
}

fn same_number(number: f64, integer: u8) -> bool {
    (number - f64::from(integer)).abs() <= f64::EPSILON
}

fn usize_to_number(value: usize) -> Result<f64, Error> {
    let value =
        u32::try_from(value).map_err(|_| Error::runtime("mock buffered amount exceeds u32"))?;
    Ok(f64::from(value))
}
