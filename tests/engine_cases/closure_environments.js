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

let read = makeReader();
print(read());

let outer = function(a) {
    return function(b) {
        return function(c) {
            return a + b + c;
        };
    };
};

outer(20)(20)(2)
