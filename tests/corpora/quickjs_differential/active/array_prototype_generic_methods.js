let joined = Array.prototype.join.call({ length: 4, 0: "a", 2: null, 3: "d" }, "|");
print("join", joined);

let search = { length: 5, 0: "a", 2: "a", 3: NaN };
print(
  "search",
  Array.prototype.includes.call(search, undefined, 1),
  Array.prototype.includes.call(search, NaN),
  Array.prototype.indexOf.call(search, undefined),
  Array.prototype.lastIndexOf.call(search, "a", 3)
);

let pushed = { length: 1, 0: "head" };
let pushLength = Array.prototype.push.call(pushed, "tail", undefined);
let popped = Array.prototype.pop.call(pushed);
print("push-pop", pushLength, popped, pushed.length, pushed[0], pushed[1], "2" in pushed);

let shifted = { length: 3, 0: "a", 2: "c" };
let first = Array.prototype.shift.call(shifted);
print("shift", first, shifted.length, "0" in shifted, shifted[1], "2" in shifted);

let unshifted = { length: 2, 1: "tail" };
let unshiftLength = Array.prototype.unshift.call(unshifted, "head");
print("unshift", unshiftLength, unshifted.length, unshifted[0], "1" in unshifted, unshifted[2]);

let sliced = Array.prototype.slice.call({ length: 4, 0: "a", 2: "c", 3: "d" }, 1, 4);
print("slice", sliced.length, "0" in sliced, sliced[1], sliced[2]);

let reversed = { length: 4, 0: "a", 2: "c" };
let returned = Array.prototype.reverse.call(reversed);
print(
  "reverse",
  returned === reversed,
  reversed.length,
  "0" in reversed,
  reversed[1],
  "2" in reversed,
  reversed[3]
);
