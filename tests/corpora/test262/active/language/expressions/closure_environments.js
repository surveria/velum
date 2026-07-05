let makeCounter = function(start) {
    var value = start;
    return function(delta) {
        value = value + delta;
        return value;
    };
};

let counter = makeCounter(40);

if (counter(1) !== 41) {
    throw new Test262Error("closure did not retain the first mutation");
}

if (counter(1) !== 42) {
    throw new Test262Error("closure did not retain the second mutation");
}

let makeReader = function() {
    var value = 1;
    let read = function() {
        return value;
    };
    value = 42;
    return read;
};

if (makeReader()() !== 42) {
    throw new Test262Error("closure captured a value snapshot instead of a binding");
}

let makeIndependentCounter = function(start) {
    var value = start;
    return function() {
        value = value + 1;
        return value;
    };
};

let left = makeIndependentCounter(0);
let right = makeIndependentCounter(40);

if (left() + left() + right() !== 44) {
    throw new Test262Error("closure instances shared a function-local environment");
}

let outer = function(a) {
    return function(b) {
        return function(c) {
            return a + b + c;
        };
    };
};

outer(20)(20)(2)
