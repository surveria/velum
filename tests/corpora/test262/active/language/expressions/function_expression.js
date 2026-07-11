let value = 1;
let update = function() {
  value = value + 41;
};
update();

let sequenceTrace = "";
let sequenceValue = (
  sequenceTrace = sequenceTrace + "a",
  sequenceTrace = sequenceTrace + "b",
  value
);
if (sequenceTrace !== "ab" || sequenceValue !== 42) {
  throw new Test262Error("comma expression evaluation mismatch");
}

value;
