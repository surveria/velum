use std::{cell::RefCell, error::Error, fs::OpenOptions, rc::Rc};

use velum_fuzzilli_target::execute_for_fuzzing;

type TestResult = Result<(), Box<dyn Error>>;

#[test]
fn reports_successful_javascript_execution() -> TestResult {
    let fuzzout = OpenOptions::new().write(true).open("/dev/null")?;
    let status = execute_for_fuzzing(
        b"let value = 40; value + 2;",
        Rc::new(RefCell::new(fuzzout)),
    )?;
    if status == 0 {
        return Ok(());
    }
    Err(format!("expected successful execution status, got {status}").into())
}

#[test]
fn reports_javascript_exceptions_without_crashing_the_target() -> TestResult {
    let fuzzout = OpenOptions::new().write(true).open("/dev/null")?;
    let status = execute_for_fuzzing(
        b"throw new Error('expected failure');",
        Rc::new(RefCell::new(fuzzout)),
    )?;
    if status == 1 {
        return Ok(());
    }
    Err(format!("expected failed execution status, got {status}").into())
}

#[test]
fn supports_fuzzilli_print_callback() -> TestResult {
    let fuzzout = OpenOptions::new().write(true).open("/dev/null")?;
    let status = execute_for_fuzzing(
        b"fuzzilli('FUZZILLI_PRINT', 'velum');",
        Rc::new(RefCell::new(fuzzout)),
    )?;
    if status == 0 {
        return Ok(());
    }
    Err(format!("expected fuzzilli callback to succeed, got {status}").into())
}

#[test]
fn reports_recursive_stack_exhaustion_without_crashing_the_target() -> TestResult {
    let fuzzout = OpenOptions::new().write(true).open("/dev/null")?;
    let status = execute_for_fuzzing(
        b"function f(...args) { try { f(); } catch (error) {} } f();",
        Rc::new(RefCell::new(fuzzout)),
    )?;
    if status == 0 {
        return Ok(());
    }
    Err(format!("expected caught stack exhaustion to succeed, got {status}").into())
}

#[test]
fn reports_recursive_native_conversion_without_crashing_the_target() -> TestResult {
    let fuzzout = OpenOptions::new().write(true).open("/dev/null")?;
    let status = execute_for_fuzzing(
        b"function F() {} const v = new F(); const a = [F, v]; v.__proto__ = a; 1[a] = 2;",
        Rc::new(RefCell::new(fuzzout)),
    )?;
    if status == 1 {
        return Ok(());
    }
    Err(format!("expected failed native conversion status, got {status}").into())
}
