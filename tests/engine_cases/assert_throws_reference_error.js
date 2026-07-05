let value = 0;

assert.throws(ReferenceError, function() {
  absent = absent;
});

try {
  missing = missing;
} catch (error) {
  print(error);
  value = 42;
}

value;
