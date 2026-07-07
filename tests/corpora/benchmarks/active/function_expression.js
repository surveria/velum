var value = 0;
var update = function() {
  value = value + 1;
};

for (let index = 0; index < 32768; index = index + 1) {
  update();
  update();
}

value;
