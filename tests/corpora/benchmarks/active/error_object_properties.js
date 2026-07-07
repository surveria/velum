var value = 0;

for (let index = 0; index < 4096; index = index + 1) {
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
}

value;
