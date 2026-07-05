let value = 0;

try {
  missing = missing;
} catch (error) {
  print(error.name);
  print(error.message);
  if (error.name === "ReferenceError" && error.message === "'missing' is not defined") {
    value = 42;
  }
}

value;
