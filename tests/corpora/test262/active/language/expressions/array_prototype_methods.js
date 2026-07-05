let values = [1];
let firstPush = values.push(2, 3);
let secondPush = values.push();
let popped = values.pop("ignored");
let afterPopLength = values.length;
delete values[1];
let hole = values.pop();
let last = values.pop();
let empty = values.pop();

let side = 0;
let marker = function() {
    side = 42;
    return "ignored";
};
[7].pop(marker());

firstPush === 3 &&
    secondPush === 3 &&
    popped === 3 &&
    afterPopLength === 2 &&
    hole === undefined &&
    last === 1 &&
    empty === undefined &&
    values.length === 0 &&
    side === 42 &&
    Array.prototype.push.name === "push" &&
    Array.prototype.push.length === 1 &&
    Array.prototype.pop.name === "pop" &&
    Array.prototype.pop.length === 0 &&
    ("push" in values) &&
    ("pop" in values) ? 42 : 0
