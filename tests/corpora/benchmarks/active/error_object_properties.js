var value = 0;

try {
  missing = missing;
} catch (error) {
  if (error.name === "ReferenceError") {
    value = value + 1;
  }
  if (error.message === "'missing' is not defined") {
    value = value + 1;
  }
}

undefined;
