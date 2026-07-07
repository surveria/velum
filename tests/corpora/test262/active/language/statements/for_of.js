let values = [];
for (const item of [1, 2, 3]) {
  values.push(item);
}
if (values.join(",") !== "1,2,3") {
  throw new Test262Error("for-of array iteration mismatch");
}

let sum = 0;
for (let item of [10, 20, 30]) {
  sum = sum + item;
}
if (sum !== 60) {
  throw new Test262Error("for-of let binding mismatch");
}

let chars = [];
for (var ch of "ab") {
  chars.push(ch);
}
if (chars.join("") !== "ab") {
  throw new Test262Error("for-of string iteration mismatch");
}

var assigned = "";
for (assigned of ["p", "q"]) {}
if (assigned !== "q") {
  throw new Test262Error("for-of assignment target mismatch");
}

let braked = [];
for (const item of [1, 2, 3]) {
  if (item === 2) {
    break;
  }
  braked.push(item);
}
if (braked.join(",") !== "1") {
  throw new Test262Error("for-of break mismatch");
}

let skipped = [];
for (const item of [1, 2, 3]) {
  if (item === 2) {
    continue;
  }
  skipped.push(item);
}
if (skipped.join(",") !== "1,3") {
  throw new Test262Error("for-of continue mismatch");
}

let caught = "";
try {
  for (const item of 5) {}
} catch (error) {
  if (error instanceof TypeError) {
    caught = "TypeError";
  }
}
if (caught !== "TypeError") {
  throw new Test262Error("for-of non-iterable must throw TypeError");
}

let custom = [];
let iterable = {};
iterable[Symbol.iterator] = function () {
  let index = 0;
  return {
    next: function () {
      index = index + 1;
      return { done: index > 3, value: index * 10 };
    }
  };
};
for (const item of iterable) {
  custom.push(item);
}
if (custom.join(",") !== "10,20,30") {
  throw new Test262Error("for-of iterator protocol mismatch");
}

let closed = false;
let closable = {};
closable[Symbol.iterator] = function () {
  return {
    next: function () {
      return { done: false, value: 1 };
    },
    return: function () {
      closed = true;
      return {};
    }
  };
};
for (const item of closable) {
  break;
}
if (!closed) {
  throw new Test262Error("for-of break must close the iterator");
}

42;
