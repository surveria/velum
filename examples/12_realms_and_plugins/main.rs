use velum::{Engine, OwnedValue};

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    vm.eval("globalThis.pluginName = 'default';")?;
    let alpha = vm.create_realm()?;
    let beta = vm.create_realm()?;
    vm.eval_in_realm(
        &alpha,
        "globalThis.pluginName = 'alpha'; globalThis.pluginState = { loads: 1 };",
    )?;
    vm.eval_in_realm(
        &beta,
        "globalThis.pluginName = 'beta'; globalThis.pluginState = { loads: 1 };",
    )?;

    let default_name = vm.eval_owned("pluginName")?;
    let alpha_name = OwnedValue::try_from(vm.eval_in_realm(&alpha, "pluginName")?)?;
    let beta_name = OwnedValue::try_from(vm.eval_in_realm(&beta, "pluginName")?)?;
    let names = [default_name, alpha_name, beta_name];
    let expected = [
        OwnedValue::String("default".to_owned()),
        OwnedValue::String("alpha".to_owned()),
        OwnedValue::String("beta".to_owned()),
    ];
    if names != expected {
        return Err(format!("realm globals were not isolated: {names:?}").into());
    }
    println!("Isolated realm globals: {names:?}");

    let alpha_intrinsics = vm.eval_in_realm(
        &alpha,
        "Array === globalThis.Array && Object.getPrototypeOf([]) === Array.prototype",
    )?;
    if OwnedValue::try_from(alpha_intrinsics)? != OwnedValue::Bool(true) {
        return Err("alpha realm did not use its own coherent intrinsics".into());
    }

    tokio::task::yield_now().await;
    let mut independent = engine.create_vm();
    if independent.eval_in_realm(&alpha, "1").is_ok() {
        return Err("an independent VM accepted a foreign RealmId".into());
    }
    println!("Foreign RealmId rejected by independent VM");
    Ok(())
}
