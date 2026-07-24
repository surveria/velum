use core::mem::size_of;

use tokio::sync::Mutex;
use velum::{Engine, Error, HostInstance, HostMethodResult, OwnedValue};
use velum_tokio::VmRuntime;

const OPEN: u8 = 1;
const CLOSED: u8 = 3;
const MAX_MESSAGES: usize = 8;
const MAX_MESSAGE_BYTES: usize = 256;

#[derive(Debug)]
struct SocketState {
    ready_state: u8,
    sent_messages: Vec<String>,
}

#[velum::host_class(name = "WebSocket", rename_all = "camelCase")]
struct MockSocket {
    #[js(get)]
    url: String,
    state: Mutex<SocketState>,
}

#[velum::host_methods]
impl MockSocket {
    #[js(constructor)]
    fn connect(url: String) -> velum::Result<HostInstance<Self>> {
        let logical_bytes = size_of::<Self>()
            .checked_add(url.len())
            .and_then(|bytes| bytes.checked_add(MAX_MESSAGES.saturating_mul(MAX_MESSAGE_BYTES)))
            .ok_or_else(|| Error::limit("mock WebSocket payload size overflowed"))?;
        Ok(HostInstance::new(
            Self {
                url,
                state: Mutex::new(SocketState {
                    ready_state: OPEN,
                    sent_messages: Vec::with_capacity(MAX_MESSAGES),
                }),
            },
            logical_bytes,
        ))
    }

    #[js(method)]
    async fn send(&self, message: String) -> velum::Result<()> {
        tokio::task::yield_now().await;
        if message.len() > MAX_MESSAGE_BYTES {
            return Err(Error::runtime("mock WebSocket message is too large"));
        }
        let mut state = self.state.lock().await;
        if state.ready_state != OPEN {
            return Err(Error::runtime("mock WebSocket is not open"));
        }
        if state.sent_messages.len() >= MAX_MESSAGES {
            return Err(Error::runtime("mock WebSocket history is full"));
        }
        state.sent_messages.push(message);
        drop(state);
        Ok(())
    }

    #[js(method)]
    async fn close(&self) -> velum::Result<()> {
        self.state.lock().await.ready_state = CLOSED;
        Ok(())
    }

    #[js(method)]
    async fn ready_state(&self) -> velum::Result<f64> {
        Ok(f64::from(self.state.lock().await.ready_state))
    }

    #[js(method)]
    async fn sent_messages(&self) -> velum::Result<String> {
        Ok(self.state.lock().await.sent_messages.join(","))
    }

    #[js(method, raw)]
    fn clone_handle(&self) -> velum::Result<HostMethodResult> {
        if self.url.is_empty() {
            return Err(Error::runtime("mock WebSocket URL is unavailable"));
        }
        Ok(HostMethodResult::shared_receiver())
    }

    #[js(static_method)]
    fn state_name(state: f64) -> velum::Result<String> {
        if !state.is_finite() {
            return Err(Error::runtime("mock WebSocket state must be finite"));
        }
        let name = if (state - f64::from(OPEN)).abs() <= f64::EPSILON {
            "OPEN"
        } else if (state - f64::from(CLOSED)).abs() <= f64::EPSILON {
            "CLOSED"
        } else {
            "UNKNOWN"
        };
        Ok(name.to_owned())
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let runtime = VmRuntime::new(Engine::new())?;
    let vm = runtime
        .spawn_vm_with(velum::Vm::register_host_type::<MockSocket>)
        .await?;
    vm.run(|vm| {
        vm.eval(
            r#"
            const socket = new WebSocket("wss://example.invalid/events");
            const clone = socket.cloneHandle();
            globalThis.summary = "pending";
            (async () => {
                await socket.send("first");
                await clone.send("second");
                const openState = await socket.readyState();
                const messages = await socket.sentMessages();
                await clone.close();
                const closedState = await socket.readyState();
                globalThis.summary = [
                    socket.url,
                    WebSocket.stateName(openState),
                    messages,
                    socket === clone,
                    WebSocket.stateName(closedState),
                    "state" in socket
                ].join(" | ");
            })();
            "#,
        )?;
        Ok(())
    })
    .await?;
    vm.wait_idle().await?;
    let summary = vm
        .run(|vm| {
            let OwnedValue::String(summary) = vm.eval_owned("summary")? else {
                return Err(Error::runtime("mock WebSocket summary was not a string"));
            };
            Ok(summary)
        })
        .await?;
    println!("{summary}");
    Ok(())
}
