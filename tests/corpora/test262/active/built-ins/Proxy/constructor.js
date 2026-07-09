// Proxy constructor shape and Proxy.revocable.
//
// Self-contained active fixture: evaluates to 42 on success, throws a
// Test262Error on failure, and produces no output.

function throwsType(thunk) {
  try {
    thunk();
    return false;
  } catch (error) {
    return error instanceof TypeError;
  }
}

if (
  typeof Proxy !== "function" ||
  Proxy.length !== 2 ||
  Proxy.name !== "Proxy"
) {
  throw new Test262Error("Proxy constructor metadata mismatch");
}

// new Proxy creates an object; Proxy without new and non-object operands throw.
var target = { a: 1 };
var proxy = new Proxy(target, {});
if (typeof proxy !== "object") {
  throw new Test262Error("new Proxy did not create an object");
}
if (
  !throwsType(function () { Proxy(target, {}); }) ||
  !throwsType(function () { return new Proxy(1, {}); }) ||
  !throwsType(function () { return new Proxy(target, 2); })
) {
  throw new Test262Error("Proxy construction validation mismatch");
}

// Proxy.revocable returns { proxy, revoke } and revoke disconnects the proxy.
if (Proxy.revocable.name !== "revocable" || Proxy.revocable.length !== 2) {
  throw new Test262Error("Proxy.revocable metadata mismatch");
}
var revocable = Proxy.revocable(target, {
  get: function () {
    return 7;
  }
});
if (
  typeof revocable.proxy !== "object" ||
  typeof revocable.revoke !== "function" ||
  revocable.proxy.a !== 7
) {
  throw new Test262Error("Proxy.revocable result mismatch");
}
revocable.revoke();
if (
  !throwsType(function () { return revocable.proxy.a; }) ||
  !throwsType(function () { return "a" in revocable.proxy; })
) {
  throw new Test262Error("revoked proxy did not raise TypeError");
}

42
