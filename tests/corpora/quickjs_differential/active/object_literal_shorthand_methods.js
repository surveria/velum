var name = "front-door";
var count = 40;
var camera = {
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

print(camera.name, camera.default, camera.delete, camera[7]);
print(camera.duplicate, camera.add.name, camera.nested(), "prototype" in camera.add);
