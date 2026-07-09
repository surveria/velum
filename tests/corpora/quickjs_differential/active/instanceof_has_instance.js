function Animal() {}
function Dog() {}
Dog.prototype = Object.create(Animal.prototype);
function Cat() {}
var dog = new Dog();

print("normal", dog instanceof Dog, dog instanceof Animal, dog instanceof Cat, dog instanceof Function);
print("object", {} instanceof Object, [] instanceof Object, [] instanceof Array);

var matchesForty = function () {};
Object.defineProperty(matchesForty, Symbol.hasInstance, {
    value: function (v) {
        return v === 40;
    },
});
print("custom-fn", 40 instanceof matchesForty, 41 instanceof matchesForty, {} instanceof matchesForty);

var stringMatcher = {
    [Symbol.hasInstance](v) {
        return typeof v === "string";
    },
};
print("custom-obj", "x" instanceof stringMatcher, 5 instanceof stringMatcher, null instanceof stringMatcher);

var alwaysTruthy = { [Symbol.hasInstance]() { return 1; } };
var alwaysFalsy = { [Symbol.hasInstance]() { return 0; } };
print("truthy", null instanceof alwaysTruthy, undefined instanceof alwaysTruthy);
print("falsy", dog instanceof alwaysFalsy, {} instanceof alwaysFalsy);

var counter = 0;
var counting = { [Symbol.hasInstance]() { counter++; return counter === 1; } };
print("effect", {} instanceof counting, {} instanceof counting, counter);

var subclassCheck = new Dog();
print("subclass", subclassCheck instanceof Animal, subclassCheck instanceof Dog);

var errors = "";
try { ({}) instanceof 5; } catch (e) { errors += e.constructor.name + ";"; }
try { ({}) instanceof "s"; } catch (e) { errors += e.constructor.name + ";"; }
try { ({}) instanceof {}; } catch (e) { errors += e.constructor.name + ";"; }
try {
    var badHandler = {};
    Object.defineProperty(badHandler, Symbol.hasInstance, { value: 5 });
    ({}) instanceof badHandler;
} catch (e) {
    errors += e.constructor.name + ";";
}
print("errors", errors);
