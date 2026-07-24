#![cfg(feature = "host-macros")]

use core::mem::size_of;

use parking_lot::Mutex;
use velum::{Engine, HostClassDefinition, HostInstance, HostMethodResult, OwnedValue, Result};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

#[velum::host_class(name = "MacroSocket", rename_all = "camelCase")]
struct MacroSocket {
    #[js(get)]
    public_url: String,
    secret: String,
    messages: Mutex<Vec<String>>,
}

#[velum::host_methods]
impl MacroSocket {
    #[js(constructor)]
    fn connect(public_url: String) -> Result<HostInstance<Self>> {
        let secret = format!("secret:{public_url}");
        let logical_bytes = size_of::<Self>()
            .checked_add(public_url.len())
            .and_then(|size| size.checked_add(secret.len()))
            .ok_or_else(|| velum::Error::limit("macro socket payload size overflowed"))?;
        Ok(HostInstance::new(
            Self {
                public_url,
                secret,
                messages: Mutex::new(Vec::new()),
            },
            logical_bytes,
        ))
    }

    #[js(method)]
    fn send(&self, message: String) -> Result<()> {
        self.messages.lock().push(message);
        Ok(())
    }

    #[js(method, name = "revealSecret")]
    fn reveal_secret(&self, prefix: String) -> Result<String> {
        Ok(format!("{prefix}:{}", self.secret))
    }

    #[js(getter)]
    fn sent_count(&self) -> Result<f64> {
        let count = u32::try_from(self.messages.lock().len())
            .map_err(|_| velum::Error::limit("macro socket message count exceeds u32"))?;
        Ok(f64::from(count))
    }

    #[js(method)]
    async fn describe(&self, suffix: String) -> Result<String> {
        core::future::ready(()).await;
        Ok(format!("{}:{suffix}", self.public_url))
    }

    #[js(method, raw, name = "cloneHandle")]
    fn clone_handle(&self) -> Result<HostMethodResult> {
        Ok(HostMethodResult::shared_receiver())
    }

    #[js(static_method, name = "kind")]
    fn kind() -> Result<String> {
        Ok("macro".to_owned())
    }
}

#[test]
fn generated_host_class_exports_only_annotated_surface() -> TestResult {
    let mut vm = Engine::new().create_vm();
    vm.register_host_type::<MacroSocket>()?;
    vm.eval(
        r#"
        const socket = new MacroSocket("wss://example.invalid");
        socket.send("first");
        const clone = socket.cloneHandle();
        globalThis.syncSummary = [
            socket.publicUrl,
            socket.sentCount,
            clone.sentCount,
            MacroSocket.kind(),
            socket.revealSecret("visible"),
            "secret" in socket,
            "messages" in socket
        ].join("|");
        globalThis.asyncSummary = "pending";
        socket.describe("ready").then(value => {
            globalThis.asyncSummary = value;
        });
        "#,
    )?;

    ensure_owned(
        vm.eval_owned("syncSummary")?,
        OwnedValue::String(
            "wss://example.invalid|1|1|macro|visible:secret:wss://example.invalid|false|false"
                .to_owned(),
        ),
    )?;

    let mut context = core::task::Context::from_waker(core::task::Waker::noop());
    let polled = vm.poll_host_futures(&mut context)?;
    if polled.completed() != 1 {
        return Err(format!("expected one completed macro host future, got {polled:?}").into());
    }
    let completed_jobs = vm.run_jobs()?;
    if completed_jobs != 1 {
        return Err(format!("expected one Promise reaction, got {completed_jobs}").into());
    }
    ensure_owned(
        vm.eval_owned("asyncSummary")?,
        OwnedValue::String("wss://example.invalid:ready".to_owned()),
    )
}

#[test]
fn generated_definition_uses_shared_payloads() -> TestResult {
    let class = <MacroSocket as HostClassDefinition>::host_class();
    let mut vm = Engine::new().create_vm();
    vm.register_host_class(class)?;
    let value = vm.eval_owned("new MacroSocket('shared') instanceof MacroSocket")?;
    ensure_owned(value, OwnedValue::Bool(true))
}

fn ensure_owned(actual: OwnedValue, expected: OwnedValue) -> TestResult {
    if actual != expected {
        return Err(format!("expected {expected:?}, got {actual:?}").into());
    }
    Ok(())
}
