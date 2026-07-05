var counter = 0;
var step = function(value) {
  return value + 1;
};
var index = 0;
while (index < 200) {
  counter = step(counter);
  index = index + 1;
}
counter;
