let global = 1;
let add = function(left, right) {
    var local = left + right;
    return local;
};
let missing = function(value) {
    return value;
};
let first = function(value) {
    return value;
};
let duplicate = function(value, value) {
    return value;
};
let ignored = 0;
let result = add(40, 2);

if (result !== 42) {
    throw new Test262Error("function parameters were not bound");
}

if (missing() !== undefined) {
    throw new Test262Error("missing function argument did not become undefined");
}

if (first(7, ignored = 99) !== 7) {
    throw new Test262Error("extra function argument changed the first parameter");
}

if (ignored !== 99) {
    throw new Test262Error("extra function argument was not evaluated");
}

if (duplicate(1, 2) !== 2) {
    throw new Test262Error("duplicate parameter did not use the last value");
}

assert.throws(ReferenceError, function() {
    local = local;
});

let bump = function(delta) {
    global = global + delta;
    return global;
};

if (bump(41) !== 42) {
    throw new Test262Error("function assignment did not update global fallback");
}

result
