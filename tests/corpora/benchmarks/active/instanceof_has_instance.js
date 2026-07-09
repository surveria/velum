function Animal() {}
function Dog() {}
Dog.prototype = Object.create(Animal.prototype);

var evenMatcher = { [Symbol.hasInstance](v) { return (v & 1) === 0; } };
var total = 0;

for (var round = 0; round < 8000; round++) {
    var dog = new Dog();
    if (dog instanceof Dog) {
        total += 1;
    }
    if (dog instanceof Animal) {
        total += 1;
    }
    if ((dog instanceof Function) === false) {
        total += 1;
    }
    if (round instanceof evenMatcher) {
        total += 1;
    }
    if ({} instanceof Object) {
        total += 1;
    }
}

total
