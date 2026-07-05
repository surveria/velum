let marker = "outer";
let value = 0;
try {
  throw "boom";
} catch {
  let marker = "inner";
  value = 42;
}
marker === "outer" ? value : 0
