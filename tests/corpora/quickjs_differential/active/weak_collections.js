var objectKey = {};
var symbolKey = Symbol("weak");
var wm = new WeakMap();
wm.set(objectKey, "object").set(symbolKey, "symbol");
print(wm.get(objectKey), wm.get(symbolKey), wm.has(objectKey), wm.has({}));
print(wm.delete(objectKey), wm.has(objectKey), wm.delete(objectKey));
print(wm.get(1) === undefined, wm.has(1), wm.delete(1));

var setKey = {};
var setSymbol = Symbol("set");
var ws = new WeakSet();
ws.add(setKey).add(setSymbol);
print(ws.has(setKey), ws.has(setSymbol), ws.has({}));
print(ws.delete(setKey), ws.has(setKey), ws.delete(setKey));
print(ws.has("x"), ws.delete("x"));

var seededKey = {};
var seededSetKey = {};
print(new WeakMap([[seededKey, 9]]).get(seededKey), new WeakSet([seededSetKey]).has(seededSetKey));

function errorName(callback) {
  try {
    callback();
    return "none";
  } catch (error) {
    return error instanceof TypeError ? "TypeError" : "other";
  }
}

print(errorName(function () { WeakMap(); }), errorName(function () { WeakSet(); }));
print(errorName(function () { wm.set(1, 2); }), errorName(function () { ws.add(1); }));
print(errorName(function () { WeakMap.prototype.get.call(ws, {}); }));
print(new WeakMap() instanceof WeakMap, new WeakSet() instanceof WeakSet);
print(WeakMap.prototype.size === undefined, WeakSet.prototype.clear === undefined);
