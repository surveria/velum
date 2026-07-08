var m = new Map();
m.set("a", 1).set("b", 2);
if (m.size !== 2 || m.get("a") !== 1 || m.get("b") !== 2 || m.get("c") !== undefined) {
  throw new Test262Error("Map basic entry mismatch");
}
if (!m.has("a") || m.has("z")) {
  throw new Test262Error("Map has mismatch");
}

var seeded = new Map([["x", 10], ["y", 20]]);
if (seeded.size !== 2 || seeded.get("x") !== 10 || seeded.get("y") !== 20) {
  throw new Test262Error("Map iterable seeding mismatch");
}

var svz = new Map();
svz.set(NaN, "nan");
svz.set(0, "zero");
svz.set(-0, "negzero");
if (svz.size !== 2 || svz.get(NaN) !== "nan" || svz.get(0) !== "negzero") {
  throw new Test262Error("SameValueZero mismatch");
}

var key = {};
var ident = new Map();
ident.set(key, "held");
if (ident.get(key) !== "held" || ident.get({}) !== undefined) {
  throw new Test262Error("object key identity mismatch");
}
if (!ident.delete(key) || ident.size !== 0 || ident.delete(key)) {
  throw new Test262Error("Map delete mismatch");
}

var s = new Set([1, 2, 2, 3]);
if (s.size !== 3 || !s.has(2) || s.has(9)) {
  throw new Test262Error("Set seeding mismatch");
}
s.add(4).add(1);
if (s.size !== 4 || !s.delete(1) || s.size !== 3) {
  throw new Test262Error("Set mutation mismatch");
}

var order = "";
var fe = new Map([["a", 1], ["b", 2]]);
fe.forEach(function (value, key2, map) {
  order = order + key2 + value + (map === fe ? "!" : "?");
});
if (order !== "a1!b2!") {
  throw new Test262Error("forEach order mismatch");
}

var acc = "";
for (var pair of fe) {
  acc = acc + pair[0] + "=" + pair[1] + ";";
}
var letters = new Set(["p", "q"]);
for (var v of letters) {
  acc = acc + v;
}
if (acc !== "a=1;b=2;pq") {
  throw new Test262Error("for-of mismatch");
}

var it = fe.keys();
var r1 = it.next();
var r2 = it.next();
var r3 = it.next();
if (r1.value !== "a" || r1.done || r2.value !== "b" || r2.done || r3.value !== undefined || !r3.done) {
  throw new Test262Error("iterator protocol mismatch");
}

var caught = "";
try {
  Map();
} catch (error) {
  if (error instanceof TypeError) {
    caught = "TypeError";
  }
}
if (caught !== "TypeError") {
  throw new Test262Error("Map call without new must throw TypeError");
}

if (Object.getPrototypeOf(new Map()) !== Map.prototype || !(new Set() instanceof Set)) {
  throw new Test262Error("prototype identity mismatch");
}

var str = new Set("aba");
if (str.size !== 2 || !str.has("a") || !str.has("b")) {
  throw new Test262Error("string seeding mismatch");
}

42;
