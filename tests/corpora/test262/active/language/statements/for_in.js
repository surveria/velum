let object = { first: 1, second: 2, third: 3 };
delete object.second;
object.second = 20;

let seen = "";
for (let key in object) {
    seen = seen + key + ":" + object[key] + ";";
}
if (seen !== "first:1;third:3;second:20;" || typeof key !== "undefined") {
    throw new Test262Error("for-in let binding or object order was unexpected");
}

let values = [10, 20];
values[3] = 40;
let indexes = "";
for (const index in values) {
    indexes = indexes + index + "=" + values[index] + ";";
}
if (indexes !== "0=10;1=20;3=40;" || typeof index !== "undefined") {
    throw new Test262Error("for-in const binding or array key order was unexpected");
}

var hoisted = "start";
for (var name in { alpha: 1, beta: 2 }) {
    hoisted = name;
}
if (hoisted !== "beta" || name !== "beta") {
    throw new Test262Error("for-in var binding was unexpected");
}

let first = function() { return "none"; };
let second = function() { return "none"; };
let count = 0;
for (const key in { alpha: 1, beta: 2 }) {
    if (count === 0) {
        first = function() { return key; };
    }
    if (count === 1) {
        second = function() { return key; };
    }
    count = count + 1;
}
if (first() !== "alpha" || second() !== "beta") {
    throw new Test262Error("for-in const binding capture was unexpected");
}

42
