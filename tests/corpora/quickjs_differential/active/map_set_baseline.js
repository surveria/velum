var m = new Map();
m.set("a", 1).set("b", 2);
print(m.size, m.get("a"), m.get("b"), m.get("c") === undefined, m.has("a"), m.has("z"));

var seeded = new Map([["x", 10], ["y", 20]]);
print(seeded.size, seeded.get("x"), seeded.get("y"));

var svz = new Map();
svz.set(NaN, "nan");
svz.set(0, "zero");
svz.set(-0, "negzero");
print(svz.size, svz.get(NaN), svz.get(0));

var key = {};
var ident = new Map();
ident.set(key, "held");
print(ident.get(key), ident.get({}) === undefined, ident.delete(key), ident.size, ident.delete(key));

var s = new Set([1, 2, 2, 3]);
print(s.size, s.has(2), s.has(9));
s.add(4).add(1);
print(s.size, s.delete(1), s.size);

var fe = new Map([["a", 1], ["b", 2]]);
var order = "";
fe.forEach(function (value, key2, map) {
  order = order + key2 + value + (map === fe ? "!" : "?");
});
print(order);

var acc = "";
for (var pair of fe) {
  acc = acc + pair[0] + "=" + pair[1] + ";";
}
var letters = new Set(["p", "q"]);
for (var v of letters) {
  acc = acc + v;
}
print(acc);

var it = fe.keys();
var r1 = it.next();
var r2 = it.next();
var r3 = it.next();
print(r1.value, r1.done, r2.value, r2.done, r3.value === undefined, r3.done);

print([...new Set([5, 6])].length, [...new Map([["z", 9]]).values()][0], [...fe.entries()][1][1]);

try {
  Map();
} catch (error) {
  print("map-call", error instanceof TypeError);
}
try {
  new Map([1]);
} catch (error) {
  print("bad-entry", error instanceof TypeError);
}

print(Object.getPrototypeOf(new Map()) === Map.prototype, new Map() instanceof Map, new Set() instanceof Set, new Set() instanceof Map);

var str = new Set("aba");
print(str.size, str.has("a"), str.has("b"), str.has("c"));

var cleared = new Map([["q", 1]]);
cleared.clear();
print(cleared.size, cleared.get("q") === undefined);
