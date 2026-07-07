var i = 0;
var total = 0;

while (i < 2048) {
  var name = "front-door";
  var count = 40;
  var camera = {
    name,
    count,
    default: 1,
    7: 2,
    add(extra) {
      return this.count + extra;
    },
  };

  if (camera.name === "front-door") {
    total = total + 1;
  }
  if (camera.default === 1 && camera[7] === 2) {
    total = total + 1;
  }
  if (camera.add(2) === 42) {
    total = total + 1;
  }

  i = i + 1;
}

total;
