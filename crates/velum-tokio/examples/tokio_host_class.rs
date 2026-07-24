use std::mem::size_of;

use tokio::sync::Mutex;
use velum::{Engine, HostInstance, OwnedValue};
use velum_tokio::VmRuntime;

#[velum::host_class(name = "WebSocket", rename_all = "camelCase")]
struct WebSocket {
    #[js(get)]
    public_url: String,
    transport_token: String,
    messages: Mutex<Vec<String>>,
}

#[velum::host_methods]
impl WebSocket {
    #[js(constructor)]
    fn connect(public_url: String) -> velum::Result<HostInstance<Self>> {
        let transport_token = format!("transport:{public_url}");
        let logical_bytes = size_of::<Self>()
            .checked_add(public_url.len())
            .and_then(|bytes| bytes.checked_add(transport_token.len()))
            .ok_or_else(|| velum::Error::limit("WebSocket payload size overflowed"))?;
        Ok(HostInstance::new(
            Self {
                public_url,
                transport_token,
                messages: Mutex::new(Vec::new()),
            },
            logical_bytes,
        ))
    }

    #[js(method)]
    async fn send(&self, message: String) -> velum::Result<String> {
        tokio::task::yield_now().await;
        self.messages.lock().await.push(message.clone());
        Ok(format!("{}:{message}", self.transport_token))
    }

    #[js(method)]
    async fn sent_count(&self) -> velum::Result<f64> {
        let count = u32::try_from(self.messages.lock().await.len())
            .map_err(|_error| velum::Error::limit("WebSocket message count exceeds u32"))?;
        Ok(f64::from(count))
    }
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let runtime = VmRuntime::builder(Engine::new())
        .worker_threads(2)
        .build()?;
    let vm = runtime
        .spawn_vm_with(velum::Vm::register_host_type::<WebSocket>)
        .await?;

    vm.run(|vm| {
        vm.eval(
            r#"
            const socket = new WebSocket("wss://example.invalid");
            globalThis.summary = "pending";
            socket.send("hello").then(async receipt => {
                const count = await socket.sentCount();
                globalThis.summary = [
                    socket.publicUrl,
                    receipt,
                    count,
                    "transport_token" in socket,
                    "messages" in socket
                ].join("|");
            });
            "#,
        )?;
        Ok(())
    })
    .await?;
    vm.wait_idle().await?;

    let summary = vm
        .run(|vm| {
            let OwnedValue::String(summary) = vm.eval_owned("summary")? else {
                return Err(velum::Error::runtime(
                    "example summary did not become a string",
                ));
            };
            Ok(summary)
        })
        .await?;
    println!("{summary}");
    Ok(())
}
