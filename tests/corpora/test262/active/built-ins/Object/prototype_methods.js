// Coverage for Object.prototype core methods and Object.fromEntries.
// Evaluates to 42 on full spec conformance, otherwise 0.

var op = Object.prototype;
var tagged = Object.create(op);
tagged[Symbol.toStringTag] = "Custom";

var created = Object.create(op);
var proto = Object.create(op);
var child = Object.create(proto);

var entries = Object.fromEntries([
    ["a", 1],
    ["b", 2],
    ["c", 3],
]);

op.toString.call([]) === "[object Array]" &&
    op.toString.call(null) === "[object Null]" &&
    op.toString.call(undefined) === "[object Undefined]" &&
    op.toString.call(42) === "[object Number]" &&
    op.toString.call("s") === "[object String]" &&
    op.toString.call(true) === "[object Boolean]" &&
    op.toString.call(function () {}) === "[object Function]" &&
    op.toString.call(new Date()) === "[object Date]" &&
    op.toString.call(/x/) === "[object RegExp]" &&
    op.toString.call(new Error("e")) === "[object Error]" &&
    op.toString.call(created) === "[object Object]" &&
    op.toString.call(tagged) === "[object Custom]" &&
    created.toString() === "[object Object]" &&
    op.valueOf.call(5) instanceof Number &&
    typeof created.valueOf === "function" &&
    proto.isPrototypeOf(child) === true &&
    child.isPrototypeOf(proto) === false &&
    op.isPrototypeOf(child) === true &&
    op.isPrototypeOf.call(op, {}) === true &&
    entries.a === 1 &&
    entries.b === 2 &&
    entries.c === 3 &&
    op.toLocaleString.call(created) === "[object Object]" &&
    op.toString.length === 0 &&
    op.valueOf.length === 0 &&
    op.toLocaleString.length === 0 &&
    op.isPrototypeOf.length === 1 &&
    Object.fromEntries.length === 1 &&
    op.toString.name === "toString" &&
    op.isPrototypeOf.name === "isPrototypeOf" &&
    Object.fromEntries.name === "fromEntries"
    ? 42
    : 0
