let object = {
    first: 40,
    nested: { value: 2 },
    duplicate: 1,
    duplicate: 41,
};

if (object.first + object.nested.value !== 42) {
    throw new Test262Error("object literal properties were not readable");
}

if (object.missing !== undefined) {
    throw new Test262Error("missing object property did not evaluate to undefined");
}

if (object.duplicate !== 41) {
    throw new Test262Error("duplicate object literal property did not keep the last value");
}

let assigned = object.first = 42;
if (assigned !== 42 || object.first !== 42) {
    throw new Test262Error("object property assignment did not store the assigned value");
}

let shared = {};
if (shared !== shared) {
    throw new Test262Error("object identity was not stable");
}

if (shared === {}) {
    throw new Test262Error("distinct object literals shared identity");
}

let order = "";
function mark(name, value) {
    order = order + name;
    return value;
}

let computed = {
    [mark("k", "front")]: mark("v", 40),
    [mark("n", "door")]: mark("w", 2),
};

if (order !== "kvnw") {
    throw new Test262Error("computed object property evaluation order was not preserved");
}

if (computed.front + computed.door !== 42) {
    throw new Test262Error("computed object properties were not readable");
}

let computedProto = { ["__proto__"]: 42 };
if (computedProto.__proto__ !== 42) {
    throw new Test262Error("computed __proto__ object property was not a data property");
}

let symbolKey = Symbol("object-literal-key");
let computedSymbol = { [symbolKey]: 42 };
if (computedSymbol[symbolKey] !== 42) {
    throw new Test262Error("computed symbol object property was not readable");
}

let make = function() {
    let state = { value: 40 };
    return function() {
        state.value = state.value + 1;
        return state.value;
    };
};

let next = make();
next();
next()
