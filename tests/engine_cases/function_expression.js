let value = 0;
let update = function() {
  value = value + 40;
  print("called");
};
update();
value = value + 2;
value;
