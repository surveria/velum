let makeCounter = function(start) {
    var value = start;
    return function(delta) {
        value = value + delta;
        return value;
    };
};

let counter = makeCounter(40);
print(counter(1));
print(counter(1));

let makeReader = function() {
    var value = 1;
    let read = function() {
        return value;
    };
    value = 42;
    return read;
};

print(makeReader()());

let makeIndependentCounter = function(start) {
    var value = start;
    return function() {
        value = value + 1;
        return value;
    };
};

let left = makeIndependentCounter(0);
let right = makeIndependentCounter(40);
print(left());
print(left());
print(right());

let outer = function(a) {
    return function(b) {
        return function(c) {
            return a + b + c;
        };
    };
};

print(outer(20)(20)(2));
