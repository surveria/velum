var next = function() {
  return 1;
};
var total = 0;

for (let index = 0; index < 32768; index = index + 1) {
  total = total + next();
  total = total + next();
}

total;
