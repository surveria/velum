let value = 0;

try {
  missing = missing;
} catch (error) {
  if (error.name !== "ReferenceError") {
    throw new Test262Error("ReferenceError name mismatch");
  }
  if (error.message !== "'missing' is not defined") {
    throw new Test262Error("ReferenceError message mismatch");
  }
  value = 42;
}

value;
