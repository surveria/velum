let value = 1;
try {
  throw "caught";
  value = 0;
} catch (error) {
  if (error === "caught") {
    value = value + 41;
  } else {
    throw new Test262Error("catch binding mismatch");
  }
}
value;
