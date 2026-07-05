let name = "front-door";
let count = 40;
let camera = {
  name,
  count,
  default: 1,
  delete: 5,
  7: 2,
  duplicate: 10,
  duplicate: 20,
  add(extra) {
    return this.count + extra;
  },
  nested() {
    return this.add(this[7]);
  },
};

let value = 0;
if (camera.name === "front-door") {
  value = value + 10;
}
if (camera.default === 1 && camera.delete === 5 && camera[7] === 2) {
  value = value + 10;
}
if (camera.duplicate === 20) {
  value = value + 10;
}
if (camera.nested() === 42) {
  value = value + 12;
}

print(camera.name, camera.default, camera.delete, camera[7], camera.add.name, "prototype" in camera.add);
value;
