use rs_quickjs::{Context, HostOperation};

const DETACH_ARRAY_BUFFER_HOST_NAME: &str = "__rsqjsTest262DetachArrayBuffer";
const CREATE_REALM_HOST_NAME: &str = "__rsqjsTest262CreateRealm";
const CREATE_IS_HTML_DDA_HOST_NAME: &str = "__rsqjsTest262CreateIsHTMLDDA";

const HOST_SOURCE: &str = r"
var $262 = {
    global: globalThis,
    detachArrayBuffer: __rsqjsTest262DetachArrayBuffer,
    IsHTMLDDA: __rsqjsTest262CreateIsHTMLDDA(),
    agent: {
        start: __rsqjsTest262AgentStart,
        broadcast: __rsqjsTest262AgentBroadcast,
        getReport: __rsqjsTest262AgentGetReport,
        sleep: __rsqjsTest262AgentSleep,
        monotonicNow: __rsqjsTest262AgentMonotonicNow
    },
    createRealm: function createRealm() {
        var realmGlobal = __rsqjsTest262CreateRealm();
        realmGlobal.$262 = {
            global: realmGlobal,
            detachArrayBuffer: $262.detachArrayBuffer,
            IsHTMLDDA: __rsqjsTest262CreateIsHTMLDDA(),
            agent: $262.agent,
            createRealm: $262.createRealm,
            evalScript: realmGlobal.eval
        };
        return { global: realmGlobal };
    },
    evalScript: function evalScript(source) {
        return (0, eval)(source);
    }
};
";

pub const STA_SOURCE: &str = r#"
let Test262Error = function Test262Error(message) {
    if (!(this instanceof Test262Error)) {
        return new Test262Error(message);
    }
    Test262Error.__rsqjsLastInstance = this;
    this.message = message || "";
};
Test262Error.prototype.toString = function () {
    return "Test262Error: " + this.message;
};
Test262Error.thrower = function (message) {
    throw new Test262Error(message);
};
let $DONOTEVALUATE = function () {
    throw new Test262Error("This statement should not be evaluated.");
};
"#;

pub const ASSERT_SOURCE: &str = r#"
function isNegativeZero(value) {
    return value === 0 && 1 / value === -Infinity;
}
function isPrimitive(value) {
    return !value || (typeof value !== "object" && typeof value !== "function");
}
function formatIdentityFreeValue(value) {
    if (typeof value === "string") {
        return typeof JSON === "undefined" ? "\"" + value + "\"" : JSON.stringify(value);
    }
    if (typeof value === "number" && isNegativeZero(value)) {
        return "-0";
    }
    if (isPrimitive(value)) {
        return String(value);
    }
    return undefined;
}
function formatSimpleValue(value) {
    let basic = formatIdentityFreeValue(value);
    if (basic !== undefined) {
        return basic;
    }
    try {
        return String(value);
    } catch (error) {
        if (error.name === "TypeError") {
            return Object.prototype.toString.call(value);
        }
        throw error;
    }
}
let assert = function assert(mustBeTrue, message) {
    if (mustBeTrue === true) {
        return;
    }
    throw new Test262Error(message || "Expected true");
};
assert.sameValue = function (actual, expected, message) {
    if (actual === expected) {
        return;
    }
    if (actual !== actual && expected !== expected) {
        return;
    }
    throw new Test262Error(message || "Expected SameValue");
};
assert.notSameValue = function (actual, unexpected, message) {
    if (actual !== unexpected) {
        return;
    }
    throw new Test262Error(message || "Expected different values");
};
function compareArray(actual, expected) {
    if (actual.length !== expected.length) {
        return false;
    }
    for (let index = 0; index < actual.length; index = index + 1) {
        if (actual[index] === expected[index]) {
            continue;
        }
        if (actual[index] !== actual[index] && expected[index] !== expected[index]) {
            continue;
        }
        return false;
    }
    return true;
}
compareArray.format = function (arrayLike) {
    return "[" + Array.prototype.map.call(arrayLike, String).join(", ") + "]";
};
assert.compareArray = function (actual, expected, message) {
    if (isPrimitive(actual) || isPrimitive(expected)) {
        throw new Test262Error(message || "Expected non-primitive array-like values");
    }
    if (compareArray(actual, expected)) {
        return;
    }
    throw new Test262Error(message || "Expected arrays to contain the same values");
};
assert.throws = function (expectedErrorConstructor, func, message) {
    let threw = false;
    let error = undefined;
    try {
        func();
    } catch (caught) {
        threw = true;
        error = caught;
    }
    if (threw !== true) {
        throw new Test262Error(message || "Expected function to throw");
    }
    if (error.constructor === expectedErrorConstructor) {
        return;
    }
    throw new Test262Error(message || "Unexpected thrown error type");
};
assert._formatIdentityFreeValue = formatIdentityFreeValue;
assert._toString = formatSimpleValue;
"#;

const DEEP_EQUAL_SOURCE: &str = r#"
function test262DeepEqual(actual, expected) {
    if (actual === expected) {
        return true;
    }
    if (actual !== actual && expected !== expected) {
        return true;
    }
    if (actual === null || expected === null ||
        typeof actual !== "object" || typeof expected !== "object") {
        return false;
    }
    let actualKeys = Object.keys(actual);
    let expectedKeys = Object.keys(expected);
    if (actualKeys.length !== expectedKeys.length) {
        return false;
    }
    for (let index = 0; index < actualKeys.length; index += 1) {
        let key = actualKeys[index];
        if (key !== expectedKeys[index] ||
            !test262DeepEqual(actual[key], expected[key])) {
            return false;
        }
    }
    return true;
}
assert.deepEqual = function (actual, expected, message) {
    if (test262DeepEqual(actual, expected)) {
        return;
    }
    throw new Test262Error(message || "Expected structurally equal values");
};
assert.deepEqual._compare = test262DeepEqual;
assert.deepEqual.format = String;
"#;

pub fn source(name: &str) -> Option<&'static str> {
    match name {
        "sta.js" => Some(STA_SOURCE),
        "assert.js" => Some(ASSERT_SOURCE),
        "deepEqual.js" => Some(DEEP_EQUAL_SOURCE),
        _ => None,
    }
}

pub fn install_host(context: &mut Context) -> rs_quickjs::Result<()> {
    context.register_host_operation(
        DETACH_ARRAY_BUFFER_HOST_NAME,
        HostOperation::DetachArrayBuffer,
    )?;
    context.register_host_operation(CREATE_REALM_HOST_NAME, HostOperation::CreateRealm)?;
    context
        .register_host_operation(CREATE_IS_HTML_DDA_HOST_NAME, HostOperation::CreateIsHtmlDda)?;
    context.eval(HOST_SOURCE).map(|_| ())
}
