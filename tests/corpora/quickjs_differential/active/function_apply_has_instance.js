function sum() {
    var total = 0;
    for (var i = 0; i < arguments.length; i++) {
        total += arguments[i];
    }
    return total;
}

print("apply", sum.apply(null, [1, 2, 3]));
print("apply-noargs", sum.apply(null));
print("apply-null", sum.apply(null, null));
print("apply-undefined", sum.apply(null, undefined));
print("apply-arraylike", sum.apply(null, { length: 4, 0: 5, 1: 6, 2: 7, 3: 8 }));
print("apply-this", (function () { return this.v; }).apply({ v: 99 }));
print("apply-bound", (function (a, b) { return a + b; }).bind(null, 10).apply(null, [5]));

var errors = "";
try { sum.apply(null, 5); } catch (e) { errors += e.constructor.name + ";"; }
try { sum.apply(null, true); } catch (e) { errors += e.constructor.name + ";"; }
try { Function.prototype.apply.call(undefined, null, []); } catch (e) { errors += e.constructor.name + ";"; }
try { Function.prototype.apply.call({}, null, []); } catch (e) { errors += e.constructor.name + ";"; }
print("errors", errors);

function Animal() {}
function Dog() {}
Dog.prototype = Object.create(Animal.prototype);
function Cat() {}
var dog = new Dog();

print("instanceof", dog instanceof Dog, dog instanceof Animal, dog instanceof Cat, dog instanceof Function);

var hasInstance = Function.prototype[Symbol.hasInstance];
print("hasInstance", hasInstance.call(Dog, dog), hasInstance.call(Animal, dog), hasInstance.call(Cat, dog));
print("hasInstance-nonobj", hasInstance.call(Dog, 42), hasInstance.call(Dog, "s"), hasInstance.call(Dog, null));
print("hasInstance-noncallable", hasInstance.call(undefined, dog), hasInstance.call(42, dog));

print(
    "meta",
    Function.prototype.apply.length,
    Function.prototype.apply.name,
    hasInstance.length,
    hasInstance.name
);
