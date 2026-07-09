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
var hasInstance = Function.prototype[Symbol.hasInstance];

var total = 0;

for (var round = 0; round < 480000; round++) {
    total += sum.apply(null, [round, round + 1, round + 2, round + 3]);
    total += sum.apply({}, { length: 3, 0: round, 1: round + 1, 2: round + 2 });

    var dog = new Dog();
    if (dog instanceof Dog) {
        total += 1;
    }
    if (dog instanceof Animal) {
        total += 1;
    }
    if (hasInstance.call(Dog, dog)) {
        total += 1;
    }
    if (!hasInstance.call(Dog, round)) {
        total += 1;
    }
}

total
