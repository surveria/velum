use velum::{Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn exposes_standard_species_accessors() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    let value = context.eval(
        r#"
        let constructors = [Array, Promise, ArrayBuffer, Map, Set, RegExp];
        let marker = {};
        let valid = constructors.every(function (constructor) {
            let descriptor = Object.getOwnPropertyDescriptor(constructor, Symbol.species);
            return typeof descriptor.get === "function" &&
                descriptor.get.length === 0 &&
                descriptor.get.name === "get [Symbol.species]" &&
                descriptor.set === undefined &&
                descriptor.enumerable === false &&
                descriptor.configurable === true &&
                descriptor.get.call(constructor) === constructor &&
                descriptor.get.call(marker) === marker;
        });
        let typedArray = Object.getPrototypeOf(Uint8Array);
        let typedArrayDescriptor = Object.getOwnPropertyDescriptor(
            typedArray,
            Symbol.species
        );
        let typedArrayValid = typeof typedArrayDescriptor.get === "function" &&
            typedArrayDescriptor.get.length === 0 &&
            typedArrayDescriptor.get.name === "get [Symbol.species]" &&
            typedArrayDescriptor.set === undefined &&
            typedArrayDescriptor.enumerable === false &&
            typedArrayDescriptor.configurable === true &&
            typedArrayDescriptor.get.call(typedArray) === typedArray &&
            Uint8Array[Symbol.species] === Uint8Array;

        Object.defineProperty(Array, Symbol.species, { value: null });
        valid && typedArrayValid && Array[Symbol.species] === null ? 42 : 0
        "#,
    )?;

    if value != Value::Number(42.0) {
        return Err(format!("expected 42, received {value:?}").into());
    }
    Ok(())
}
