// Proxy get, set, has and deleteProperty traps, plus target fallback.
//
// Self-contained active fixture: evaluates to 42 on success, throws a
// Test262Error on failure, and produces no output.

var target = { a: 1, b: 2 };
var log = [];
var proxy = new Proxy(target, {
  get: function (t, key, receiver) {
    log.push("get:" + key);
    return key === "a" ? 42 : t[key];
  },
  set: function (t, key, value) {
    log.push("set:" + key);
    t[key] = value;
    return true;
  },
  has: function (t, key) {
    log.push("has:" + key);
    return key in t;
  },
  deleteProperty: function (t, key) {
    log.push("del:" + key);
    delete t[key];
    return true;
  }
});

if (proxy.a !== 42 || proxy.b !== 2 || proxy["a"] !== 42) {
  throw new Test262Error("proxy get trap mismatch");
}

proxy.c = 3;
if (target.c !== 3) {
  throw new Test262Error("proxy set trap did not reach target");
}

if (!("a" in proxy) || "z" in proxy) {
  throw new Test262Error("proxy has trap mismatch");
}

if (!(delete proxy.b) || "b" in target) {
  throw new Test262Error("proxy deleteProperty trap mismatch");
}

if (log.join(",") !== "get:a,get:b,get:a,set:c,has:a,has:z,del:b") {
  throw new Test262Error("proxy trap invocation order mismatch: " + log.join(","));
}

// No-trap fallback goes straight to the target.
var plain = new Proxy({ x: 9 }, {});
plain.y = 10;
if (plain.x !== 9 || plain.y !== 10 || !("x" in plain) || !(delete plain.x)) {
  throw new Test262Error("proxy target fallback mismatch");
}

42
