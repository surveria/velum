var counter = 0;
var step = function(value) {
  return value + 1;
};
var index = 0;
while (index < 98304) {
  counter = step(counter);
  index = index + 1;
}
counter;
