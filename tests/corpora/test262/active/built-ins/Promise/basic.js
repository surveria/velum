if (typeof Promise !== "function") {
  throw new Test262Error("Promise binding must be a function");
}
if (Promise.name !== "Promise" || Promise.length !== 1) {
  throw new Test262Error("Promise constructor descriptors are unexpected");
}
if (typeof Promise.resolve !== "function" || Promise.resolve.length !== 1) {
  throw new Test262Error("Promise.resolve descriptors are unexpected");
}
if (typeof Promise.reject !== "function" || Promise.reject.length !== 1) {
  throw new Test262Error("Promise.reject descriptors are unexpected");
}
if (typeof Promise.prototype.then !== "function" || Promise.prototype.then.length !== 2) {
  throw new Test262Error("Promise.prototype.then descriptors are unexpected");
}
if (typeof Promise.prototype.catch !== "function" || Promise.prototype.catch.length !== 1) {
  throw new Test262Error("Promise.prototype.catch descriptors are unexpected");
}
if (Promise.prototype.constructor !== Promise) {
  throw new Test262Error("Promise.prototype.constructor mismatch");
}

(async function() {
  let fulfilled = await Promise.resolve(40).then(function(value) {
    return value + 2;
  });
  if (fulfilled !== 42) {
    throw new Test262Error("Promise.resolve reaction did not run");
  }

  let rejected = await Promise.reject("offline").catch(function(reason) {
    return reason;
  });
  if (rejected !== "offline") {
    throw new Test262Error("Promise.reject reaction did not run");
  }
  return fulfilled;
})().then(function(value) {
  print("promise-basic:" + value);
}, function(error) {
  print("promise-basic-error:" + error);
});

42
