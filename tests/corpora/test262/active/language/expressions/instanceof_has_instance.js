// Coverage for the instanceof operator consulting Symbol.hasInstance.
// Evaluates to 42 on full spec conformance, otherwise 0.

function Animal() {}
function Dog() {}
Dog.prototype = Object.create(Animal.prototype);
function Cat() {}
var dog = new Dog();

var matchesForty = function () {};
Object.defineProperty(matchesForty, Symbol.hasInstance, {
    value: function (v) {
        return v === 40;
    },
});

var stringMatcher = {
    [Symbol.hasInstance](v) {
        return typeof v === "string";
    },
};

var alwaysTruthy = {
    [Symbol.hasInstance]() {
        return 1;
    },
};

var nonObjectError = false;
try {
    ({}) instanceof 5;
} catch (error) {
    nonObjectError = error instanceof TypeError;
}

var noHandlerError = false;
try {
    ({}) instanceof {};
} catch (error) {
    noHandlerError = error instanceof TypeError;
}

dog instanceof Dog &&
    dog instanceof Animal &&
    (dog instanceof Cat) === false &&
    (dog instanceof Function) === false &&
    (40 instanceof matchesForty) === true &&
    (41 instanceof matchesForty) === false &&
    ({}) instanceof matchesForty === false &&
    ("x" instanceof stringMatcher) === true &&
    (5 instanceof stringMatcher) === false &&
    (null instanceof alwaysTruthy) === true &&
    (undefined instanceof alwaysTruthy) === true &&
    nonObjectError &&
    noHandlerError
    ? 42
    : 0
