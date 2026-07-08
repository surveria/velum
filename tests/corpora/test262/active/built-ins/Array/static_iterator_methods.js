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

mapped.join("|") === "10|21|32" &&
    arrayLike.length === 3 &&
    arrayLike[0] === "a" &&
    arrayLike[1] === undefined &&
    arrayLike[2] === "c" &&
    text.join("|") === "x|y" &&
    ofValues.length === 2 &&
    ofValues[1] === "right" &&
    custom instanceof Capture &&
    custom.constructedLength === 2 &&
    custom[0] === 7 &&
    customFrom instanceof Capture &&
    customFrom.constructedLength === 1 &&
    customFrom[0] === "z" &&
    Array.prototype[Symbol.iterator] === Array.prototype.values &&
    iterator[Symbol.iterator]() === iterator &&
    keys.next().value === 0 &&
    keys.next().value === 1 &&
    keys.next().value === 2 &&
    keys.next().done === true &&
    entries.next().value.join(":") === "0:a" &&
    iterator.next().value === "a" &&
    iterator.next().value === "b" &&
    iterator.next().value === "c" &&
    iterator.next().done === true &&
    objectIterator.next().value === "o" &&
    objectIterator.next().value === undefined &&
    objectIterator.next().value === "p" ? 42 : 0
