// Coverage for Function.prototype.apply and Function.prototype[Symbol.hasInstance].
// Evaluates to 42 on full spec conformance, otherwise 0.

function sum() {
    var total = 0;
    for (var i = 0; i < arguments.length; i++) {
        total += arguments[i];
    }
    return total;
}

function Animal() {}
function Dog() {}
Dog.prototype = Object.create(Animal.prototype);
var dog = new Dog();

var hasInstance = Function.prototype[Symbol.hasInstance];

var applyNonObject = false;
try {
    sum.apply(null, 5);
} catch (error) {
    applyNonObject = error instanceof TypeError;
}

var applyNonCallable = false;
try {
    Function.prototype.apply.call(undefined, null, []);
} catch (error) {
    applyNonCallable = error instanceof TypeError;
}

sum.apply(null, [1, 2, 3]) === 6 &&
    sum.apply(null) === 0 &&
    sum.apply(null, null) === 0 &&
    sum.apply(null, undefined) === 0 &&
    sum.apply(null, { length: 3, 0: 10, 1: 20, 2: 30 }) === 60 &&
    (function () {
        return this.marker;
    }).apply({ marker: 7 }) === 7 &&
    applyNonObject &&
    applyNonCallable &&
    dog instanceof Dog &&
    dog instanceof Animal &&
    (dog instanceof Function) === false &&
    hasInstance.call(Dog, dog) === true &&
    hasInstance.call(Animal, dog) === true &&
    hasInstance.call(Dog, {}) === false &&
    hasInstance.call(Dog, 42) === false &&
    hasInstance.call(undefined, dog) === false &&
    Function.prototype.apply.length === 2 &&
    Function.prototype.apply.name === "apply" &&
    hasInstance.length === 1 &&
    hasInstance.name === "[Symbol.hasInstance]"
    ? 42
    : 0
