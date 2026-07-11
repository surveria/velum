let value = 0;
let update = function() {
  value = value + 1;
  print(value);
};
print(update());
update();
print(value);

let sequenceTrace = "";
let sequenceValue = (
  sequenceTrace = sequenceTrace + "a",
  sequenceTrace = sequenceTrace + "b",
  value
);
print(sequenceTrace, sequenceValue);
