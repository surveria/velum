let name = "front-door";
let count = 40;
let camera = {
  name,
  count,
  default: 1,
  delete: 5,
  7: 2,
  duplicate: 10,
  duplicate: 20,
  add(extra) {
    return this.count + extra;
  },
  nested() {
    return this.add(this[7]);
  },
};

if (camera.name !== "front-door") {
  throw new Test262Error("shorthand property mismatch");
}
if (camera.default !== 1) {
  throw new Test262Error("keyword property mismatch");
}
if (camera.delete !== 5) {
  throw new Test262Error("keyword member mismatch");
}
if (camera[7] !== 2) {
  throw new Test262Error("numeric property mismatch");
}
if (camera.duplicate !== 20) {
  throw new Test262Error("duplicate property mismatch");
}
if (camera.nested() !== 42) {
  throw new Test262Error("concise method mismatch");
}
if ("prototype" in camera.add) {
  throw new Test262Error("concise method should not expose prototype");
}

camera.nested();
