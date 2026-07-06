async function answer() {
  let base = await Promise.resolve(40);
  return base + 2;
}

let resolved = await answer();
if (resolved !== 42) {
  throw new Test262Error("async function await result mismatch");
}

async function passthrough(value) {
  return await value;
}

let plain = await passthrough("camera");
if (plain !== "camera") {
  throw new Test262Error("await plain value mismatch");
}

42
