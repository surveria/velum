// Proxy getPrototypeOf, setPrototypeOf, isExtensible and preventExtensions
// traps, plus target fallback.
//
// Self-contained active fixture: evaluates to 42 on success, throws a
// Test262Error on failure, and produces no output.

var proto = { marker: 7 };
var target = Object.create(proto);
var log = [];
var proxy = new Proxy(target, {
  getPrototypeOf: function (t) {
    log.push("gpo");
    return Object.getPrototypeOf(t);
  },
  setPrototypeOf: function (t, p) {
    log.push("spo");
    Object.setPrototypeOf(t, p);
    return true;
  },
  isExtensible: function (t) {
    log.push("ext");
    return Object.isExtensible(t);
  },
  preventExtensions: function (t) {
    log.push("prev");
    Object.preventExtensions(t);
    return true;
  }
});

if (Object.getPrototypeOf(proxy) !== proto) {
  throw new Test262Error("proxy getPrototypeOf trap mismatch");
}
if (Object.isExtensible(proxy) !== true) {
  throw new Test262Error("proxy isExtensible trap mismatch");
}

var replacement = { swapped: true };
if (Object.setPrototypeOf(proxy, replacement) !== proxy) {
  throw new Test262Error("proxy setPrototypeOf did not return the proxy");
}
if (Object.getPrototypeOf(proxy) !== replacement) {
  throw new Test262Error("proxy setPrototypeOf trap did not update the prototype");
}

Object.preventExtensions(proxy);
if (Object.isExtensible(proxy) !== false) {
  throw new Test262Error("proxy preventExtensions trap mismatch");
}

if (log.join(",") !== "gpo,ext,spo,gpo,prev,ext") {
  throw new Test262Error("proxy prototype trap order mismatch: " + log.join(","));
}

// No-trap fallback reflects the target directly.
var plain = new Proxy(Object.create(proto), {});
if (Object.getPrototypeOf(plain) !== proto || Object.isExtensible(plain) !== true) {
  throw new Test262Error("proxy prototype fallback mismatch");
}

42
