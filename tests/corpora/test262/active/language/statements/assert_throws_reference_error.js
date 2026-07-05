var first, second = true, third = second ? 40 : 0;

assert.throws(ReferenceError, function() {
  missing = missing;
});

if (first !== undefined) {
  throw new Test262Error("var declaration was not hoisted to undefined");
}

third = third + (second ? 2 : 0);
third;
