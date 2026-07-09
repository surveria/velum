// Reflect namespace shape: it is a non-callable ordinary object exposing the
// standard static methods with the correct name and length metadata, plus a
// non-writable, non-enumerable, configurable Symbol.toStringTag of "Reflect".
//
// Self-contained active fixture: it evaluates to 42 on success and throws a
// Test262Error (a dead branch that never resolves the identifier on success)
// on failure, without producing any output.

var methods = [
  ["apply", 3],
  ["construct", 2],
  ["defineProperty", 3],
  ["deleteProperty", 2],
  ["get", 2],
  ["getOwnPropertyDescriptor", 2],
  ["getPrototypeOf", 1],
  ["has", 2],
  ["isExtensible", 1],
  ["ownKeys", 1],
  ["preventExtensions", 1],
  ["set", 3],
  ["setPrototypeOf", 2]
];

var metadataOk = typeof Reflect === "object";
for (var i = 0; i < methods.length; i++) {
  var fn = Reflect[methods[i][0]];
  metadataOk =
    metadataOk &&
    typeof fn === "function" &&
    fn.name === methods[i][0] &&
    fn.length === methods[i][1];
}

if (!metadataOk) {
  throw new Test262Error("Reflect method metadata mismatch");
}

if (Object.prototype.toString.call(Reflect) !== "[object Reflect]") {
  throw new Test262Error("Reflect Symbol.toStringTag mismatch");
}

var tagDescriptor = Object.getOwnPropertyDescriptor(Reflect, Symbol.toStringTag);
if (
  tagDescriptor.value !== "Reflect" ||
  tagDescriptor.writable !== false ||
  tagDescriptor.enumerable !== false ||
  tagDescriptor.configurable !== true
) {
  throw new Test262Error("Reflect[Symbol.toStringTag] descriptor mismatch");
}

// Reflect itself is not callable.
var reflectNotCallable = false;
try {
  Reflect();
} catch (error) {
  reflectNotCallable = error instanceof TypeError;
}
if (!reflectNotCallable) {
  throw new Test262Error("Reflect should not be callable");
}

42
