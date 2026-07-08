let mapped = Array.from([1, 2, 3], function(value, index) {
    return value * 10 + index;
});
let arrayLike = Array.from({ 0: "a", 2: "c", length: 3 });
let text = Array.from("xy");
let ofValues = Array.of("left", "right");

function Capture(length) {
    this.constructedLength = length;
}

let custom = Array.of.call(Capture, 7, 8);
let customFrom = Array.from.call(Capture, { 0: "z", length: 1 });
let values = ["a", "b"];
let keys = values.keys();
let entries = values.entries();
let iterator = values[Symbol.iterator]();
values.push("c");
let objectIterator = Array.prototype.values.call({ 0: "o", 2: "p", length: 3 });

print(mapped.join("|"));
print(arrayLike.length, arrayLike[0], arrayLike[1], arrayLike[2]);
print(text.join("|"), ofValues.length, ofValues[1]);
print(
    custom instanceof Capture,
    custom.constructedLength,
    custom[0],
    customFrom instanceof Capture,
    customFrom.constructedLength,
    customFrom[0]
);
print(Array.prototype[Symbol.iterator] === Array.prototype.values);
print(iterator[Symbol.iterator]() === iterator);
print(keys.next().value, keys.next().value, keys.next().value, keys.next().done);
print(entries.next().value.join(":"));
print(iterator.next().value, iterator.next().value, iterator.next().value, iterator.next().done);
print(objectIterator.next().value, objectIterator.next().value, objectIterator.next().value);
