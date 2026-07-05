let first = Math.random();
let second = Math.random();

let metadataOk =
  Math.random.name === "random" &&
  Math.random.length === 0;

if (!metadataOk) {
  throw new Test262Error("Math.random metadata mismatch");
}

let typeOk =
  typeof first === "number" &&
  typeof second === "number" &&
  first === first &&
  second === second;

if (!typeOk) {
  throw new Test262Error("Math.random type mismatch");
}

let rangeOk =
  first >= 0 &&
  first < 1 &&
  second >= 0 &&
  second < 1;

if (!rangeOk) {
  throw new Test262Error("Math.random range mismatch");
}

42
