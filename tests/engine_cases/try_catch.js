let marker = "outer";
let value = 0;
try {
  throw "boom";
  value = 1;
} catch (marker) {
  print(marker);
  value = 42;
}
if (marker !== "outer") {
  throw new Test262Error("catch binding leaked");
}
value;
