// Proxy defineProperty, getOwnPropertyDescriptor and ownKeys traps.
//
// Self-contained active fixture: evaluates to 42 on success, throws a
// Test262Error on failure, and produces no output.

var target = {};
var log = [];
var proxy = new Proxy(target, {
  defineProperty: function (t, key, descriptor) {
    log.push("def:" + key);
    Object.defineProperty(t, key, descriptor);
    return true;
  },
  getOwnPropertyDescriptor: function (t, key) {
    log.push("gopd:" + key);
    return Object.getOwnPropertyDescriptor(t, key);
  },
  ownKeys: function () {
    log.push("keys");
    return ["one", "two", "hidden"];
  }
});

Object.defineProperty(proxy, "one", {
  value: 1,
  enumerable: true,
  writable: true,
  configurable: true
});
Object.defineProperty(proxy, "two", {
  value: 2,
  enumerable: true,
  writable: true,
  configurable: true
});
Object.defineProperty(proxy, "hidden", {
  value: 3,
  enumerable: false,
  writable: true,
  configurable: true
});

if (target.one !== 1 || target.two !== 2 || target.hidden !== 3) {
  throw new Test262Error("proxy defineProperty trap did not reach target");
}

var descriptor = Object.getOwnPropertyDescriptor(proxy, "one");
if (
  descriptor.value !== 1 ||
  descriptor.enumerable !== true ||
  descriptor.writable !== true ||
  descriptor.configurable !== true
) {
  throw new Test262Error("proxy getOwnPropertyDescriptor trap mismatch");
}
if (Object.getOwnPropertyDescriptor(proxy, "hidden").enumerable !== false) {
  throw new Test262Error("proxy descriptor enumerability mismatch");
}

if (Object.getOwnPropertyNames(proxy).join(",") !== "one,two,hidden") {
  throw new Test262Error("proxy ownKeys (all names) mismatch");
}
if (Object.keys(proxy).join(",") !== "one,two") {
  throw new Test262Error("proxy ownKeys (enumerable) mismatch");
}

// No-trap fallback reflects the target directly.
var plain = new Proxy({ m: 1, n: 2 }, {});
if (
  Object.getOwnPropertyNames(plain).join(",") !== "m,n" ||
  Object.getOwnPropertyDescriptor(plain, "m").value !== 1 ||
  Object.getOwnPropertyDescriptor(plain, "absent") !== undefined
) {
  throw new Test262Error("proxy reflection fallback mismatch");
}

42
