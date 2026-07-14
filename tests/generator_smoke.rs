use rs_quickjs::{Error, Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

fn eval(source: &str) -> rs_quickjs::Result<Value> {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    context.eval(source)
}

fn ensure_value(actual: &Value, expected: &Value) -> TestResult {
    if actual == expected {
        return Ok(());
    }
    Err(format!("expected value {expected:?}, got {actual:?}").into())
}

fn ensure_parse_error(source: &str) -> TestResult {
    let Err(error) = eval(source) else {
        return Err(format!("expected '{source}' to fail during parsing").into());
    };
    if matches!(error, Error::Parse { .. }) {
        return Ok(());
    }
    Err(format!("expected parse error, got '{error}'").into())
}

#[test]
fn generator_declaration_yields_and_returns() -> TestResult {
    let value = eval(
        r#"
        function* values() {
            yield 40;
            return 42;
        }
        const iterator = values();
        const first = iterator.next();
        const second = iterator.next();
        first.value + ":" + first.done + ":" + second.value + ":" + second.done
        "#,
    )?;
    ensure_value(&value, &Value::from("40:false:42:true"))
}

#[test]
fn rejects_generator_declaration_early_errors() -> TestResult {
    for source in [
        "if (true) function* values() {}",
        "{ function* values() {} let values; }",
        "switch (0) { case 0: function* values() {} default: var values; }",
        "({ *values(item) { let item; } });",
    ] {
        ensure_parse_error(source)?;
    }
    Ok(())
}

#[test]
fn next_value_becomes_yield_expression_result() -> TestResult {
    let value = eval(
        r"
        const iterator = (function* () {
            const received = yield 1;
            return received + 2;
        })();
        iterator.next();
        iterator.next(40).value
        ",
    )?;
    ensure_value(&value, &Value::Number(42.0))
}

#[test]
fn generator_return_runs_finally_and_can_yield_again() -> TestResult {
    let value = eval(
        r#"
        function* values() {
            try {
                yield 1;
            } finally {
                yield 2;
            }
        }
        const iterator = values();
        iterator.next();
        const cleanup = iterator.return(40);
        const completed = iterator.next();
        cleanup.value + ":" + cleanup.done + ":" + completed.value + ":" + completed.done
        "#,
    )?;
    ensure_value(&value, &Value::from("2:false:40:true"))
}

#[test]
fn generator_throw_enters_catch() -> TestResult {
    let value = eval(
        r#"
        function* values() {
            try {
                yield 1;
            } catch (error) {
                return error + 2;
            }
        }
        const iterator = values();
        iterator.next();
        const completed = iterator.throw(40);
        completed.value + ":" + completed.done
        "#,
    )?;
    ensure_value(&value, &Value::from("42:true"))
}

#[test]
fn generator_object_method_is_iterable() -> TestResult {
    let value = eval(
        r"
        const holder = {
            *values() {
                yield 20;
                yield 22;
            }
        };
        let total = 0;
        for (const value of holder.values()) {
            total = total + value;
        }
        total
        ",
    )?;
    ensure_value(&value, &Value::Number(42.0))
}

#[test]
fn generator_prototype_inherits_the_iterator_method() -> TestResult {
    let value = eval(
        r"
        function* values() {}
        let generatorPrototype = Object.getPrototypeOf(values.prototype);
        let iteratorPrototype = Object.getPrototypeOf(generatorPrototype);
        Object.getOwnPropertySymbols(generatorPrototype).length === 1 &&
            Object.getOwnPropertySymbols(generatorPrototype)[0] === Symbol.toStringTag &&
            !Object.prototype.hasOwnProperty.call(generatorPrototype, Symbol.iterator) &&
            generatorPrototype[Symbol.iterator] === iteratorPrototype[Symbol.iterator] ? 42 : 0
        ",
    )?;
    ensure_value(&value, &Value::Number(42.0))
}

#[test]
fn evaluates_parameters_on_call_but_defers_the_body() -> TestResult {
    let value = eval(
        r#"
        let bodyCalls = 0;
        let parameterThrew = false;
        function* values(value = missing) {
            bodyCalls = bodyCalls + 1;
            yield value;
        }
        try {
            values();
        } catch (error) {
            parameterThrew = error instanceof ReferenceError;
        }
        const iterator = values(42);
        parameterThrew + ":" + bodyCalls + ":" + iterator.next().value + ":" + bodyCalls
        "#,
    )?;
    ensure_value(&value, &Value::from("true:0:42:1"))
}

#[test]
fn generator_destructuring_parameter_errors_are_catchable_on_call() -> TestResult {
    let value = eval(
        r"
        function* values({}) {}
        let caught = false;
        try {
            values(null);
        } catch (error) {
            caught = error instanceof TypeError;
        }
        caught
        ",
    )?;
    ensure_value(&value, &Value::Bool(true))
}

#[test]
fn synchronous_generators_treat_await_as_an_identifier() -> TestResult {
    let value = eval(
        r"
        var await = 40;
        var parameterValue = 0;
        var bodyValue = 0;
        class Holder {
            static {
                (function* await(value) {});
                (function*(value = await) {
                    parameterValue = value;
                    bodyValue = await + 2;
                })().next();
            }
        }
        parameterValue + bodyValue
        ",
    )?;
    ensure_value(&value, &Value::Number(82.0))
}

#[test]
fn generator_parameters_observe_array_iterator_deletion() -> TestResult {
    let value = eval(
        r"
        function* values([value]) {
            yield value;
        }
        delete Array.prototype[Symbol.iterator];
        let caught = false;
        try {
            values([42]);
        } catch (error) {
            caught = error instanceof TypeError;
        }
        caught
        ",
    )?;
    ensure_value(&value, &Value::Bool(true))
}

#[test]
fn object_spread_copies_symbol_properties_for_generators() -> TestResult {
    let value = eval(
        r#"
        const key = Symbol("generator");
        const source = {};
        source[key] = 42;
        function* values() {
            yield ({ ...source })[key];
        }
        values().next().value
        "#,
    )?;
    ensure_value(&value, &Value::Number(42.0))
}

#[test]
fn direct_eval_reports_generator_scope_conflicts_as_syntax_errors() -> TestResult {
    let value = eval(
        r#"
        function* parameterConflict(value = eval("var value = 42")) {}
        function* lexicalConflict() {
            let value;
            eval("var value");
        }
        let parameterThrew = false;
        let lexicalThrew = false;
        try {
            parameterConflict();
        } catch (error) {
            parameterThrew = error instanceof SyntaxError;
        }
        try {
            lexicalConflict().next();
        } catch (error) {
            lexicalThrew = error instanceof SyntaxError;
        }
        parameterThrew + ":" + lexicalThrew
        "#,
    )?;
    ensure_value(&value, &Value::from("true:true"))
}

#[test]
fn yield_delegate_forwards_next_and_completion_values() -> TestResult {
    let value = eval(
        r#"
        function* inner() {
            const received = yield 1;
            return received + 1;
        }
        function* outer() {
            return yield* inner();
        }
        const iterator = outer();
        const first = iterator.next();
        const completed = iterator.next(41);
        first.value + ":" + first.done + ":" + completed.value + ":" + completed.done
        "#,
    )?;
    ensure_value(&value, &Value::from("1:false:42:true"))
}

#[test]
fn yield_delegate_forwards_return_through_finally() -> TestResult {
    let value = eval(
        r#"
        function* inner() {
            try {
                yield 1;
            } finally {
                yield 2;
            }
        }
        function* outer() {
            return yield* inner();
        }
        const iterator = outer();
        iterator.next();
        const cleanup = iterator.return(40);
        const completed = iterator.next();
        cleanup.value + ":" + cleanup.done + ":" + completed.value + ":" + completed.done
        "#,
    )?;
    ensure_value(&value, &Value::from("2:false:40:true"))
}

#[test]
fn yield_delegate_forwards_throw() -> TestResult {
    let value = eval(
        r#"
        function* inner() {
            try {
                yield 1;
            } catch (error) {
                return error + 2;
            }
        }
        function* outer() {
            return yield* inner();
        }
        const iterator = outer();
        iterator.next();
        const completed = iterator.throw(40);
        completed.value + ":" + completed.done
        "#,
    )?;
    ensure_value(&value, &Value::from("42:true"))
}

#[test]
fn yield_delegate_preserves_protocol_iterator_result() -> TestResult {
    let value = eval(
        r#"
        const innerResult = { value: 42 };
        const iterable = {};
        iterable[Symbol.iterator] = function() {
            return {
                next: function() {
                    return innerResult;
                }
            };
        };
        function* values() {
            yield* iterable;
        }
        const actual = values().next();
        (actual === innerResult) + ":" + actual.done + ":" + actual.value
        "#,
    )?;
    ensure_value(&value, &Value::from("true:undefined:42"))
}

#[test]
fn yield_delegate_propagates_return_when_method_is_absent() -> TestResult {
    let value = eval(
        r#"
        let returnGets = 0;
        const iterable = {
            next: function() {
                return { value: 1, done: false };
            }
        };
        Object.defineProperty(iterable, "return", {
            get: function() {
                returnGets = returnGets + 1;
                return null;
            }
        });
        iterable[Symbol.iterator] = function() {
            return iterable;
        };
        function* values() {
            yield* iterable;
        }
        const iterator = values();
        iterator.next();
        const completed = iterator.return(42);
        completed.value + ":" + completed.done + ":" + returnGets
        "#,
    )?;
    ensure_value(&value, &Value::from("42:true:1"))
}

#[test]
fn yield_delegate_closes_iterator_when_throw_method_is_absent() -> TestResult {
    let value = eval(
        r#"
        let closed = false;
        const iterable = {
            next: function() {
                return { value: 1, done: false };
            },
            return: function() {
                closed = true;
                return { done: true };
            }
        };
        iterable[Symbol.iterator] = function() {
            return iterable;
        };
        function* values() {
            yield* iterable;
        }
        const iterator = values();
        iterator.next();
        let caught = false;
        try {
            iterator.throw(42);
        } catch (error) {
            caught = error instanceof TypeError;
        }
        closed + ":" + caught
        "#,
    )?;
    ensure_value(&value, &Value::from("true:true"))
}
