let value = undefined ? 1 : 40;
let ok = (value === 40) & (true ? 1 : 0);
if (ok === 1) {
  value = value + 2;
} else {
  throw new Test262Error("conditional or bitwise and mismatch");
}
value;
