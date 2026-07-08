let joined = Array.prototype.join.call({ length: 4, 0: "a", 2: null, 3: "d" }, "|");
if (joined !== "a|||d") {
  throw new Test262Error("generic join mismatch");
}

let search = { length: 5, 0: "a", 2: "a", 3: NaN };
if (!Array.prototype.includes.call(search, undefined, 1)) {
  throw new Test262Error("generic includes must read holes as undefined");
}
if (!Array.prototype.includes.call(search, NaN)) {
  throw new Test262Error("generic includes must use SameValueZero");
}
if (Array.prototype.indexOf.call(search, undefined) !== -1) {
  throw new Test262Error("generic indexOf must skip holes");
}
if (Array.prototype.lastIndexOf.call(search, "a", 3) !== 2) {
  throw new Test262Error("generic lastIndexOf mismatch");
}

let pushed = { length: 1, 0: "head" };
let pushLength = Array.prototype.push.call(pushed, "tail", undefined);
let popped = Array.prototype.pop.call(pushed);
if (
  pushLength !== 3 ||
  popped !== undefined ||
  pushed.length !== 2 ||
  pushed[0] !== "head" ||
  pushed[1] !== "tail" ||
  "2" in pushed
) {
  throw new Test262Error("generic push/pop mismatch");
}

let shifted = { length: 3, 0: "a", 2: "c" };
let first = Array.prototype.shift.call(shifted);
if (
  first !== "a" ||
  shifted.length !== 2 ||
  "0" in shifted ||
  shifted[1] !== "c" ||
  "2" in shifted
) {
  throw new Test262Error("generic shift mismatch");
}

let unshifted = { length: 2, 1: "tail" };
let unshiftLength = Array.prototype.unshift.call(unshifted, "head");
if (
  unshiftLength !== 3 ||
  unshifted.length !== 3 ||
  unshifted[0] !== "head" ||
  "1" in unshifted ||
  unshifted[2] !== "tail"
) {
  throw new Test262Error("generic unshift mismatch");
}

let sliced = Array.prototype.slice.call({ length: 4, 0: "a", 2: "c", 3: "d" }, 1, 4);
if (sliced.length !== 3 || "0" in sliced || sliced[1] !== "c" || sliced[2] !== "d") {
  throw new Test262Error("generic slice mismatch");
}

let reversed = { length: 4, 0: "a", 2: "c" };
let returned = Array.prototype.reverse.call(reversed);
if (
  returned !== reversed ||
  reversed.length !== 4 ||
  "0" in reversed ||
  reversed[1] !== "c" ||
  "2" in reversed ||
  reversed[3] !== "a"
) {
  throw new Test262Error("generic reverse mismatch");
}

42;
