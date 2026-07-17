use std::{cell::RefCell, rc::Rc};

use velum::{Engine, OwnedValue};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut vm = Engine::new().create_vm();
    vm.register_host_function_typed("rustAdd", |call| {
        let left = call.number(0, "left")?;
        let right = call.number(1, "right")?;
        Ok(left + right)
    })?;

    let messages = Rc::new(RefCell::new(Vec::new()));
    let captured_messages = Rc::clone(&messages);
    vm.register_host_function_typed("rustGreet", move |call| {
        let name = call.string(0, "name")?;
        let greeting = format!("Hello, {name}, from Rust!");
        captured_messages.borrow_mut().push(greeting.clone());
        Ok(greeting)
    })?;

    let result = vm.eval_owned(
        r#"
        const total = rustAdd(20, 22);
        `${rustGreet("JavaScript")} Answer: ${total}`;
        "#,
    )?;
    let OwnedValue::String(result) = result else {
        return Err("expected a string result".into());
    };
    println!("{result}");
    println!("Captured by Rust: {:?}", messages.borrow().as_slice());
    Ok(())
}
