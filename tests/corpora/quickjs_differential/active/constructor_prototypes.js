let Camera = function Camera(name) {
    this.name = name;
    this.count = 40;
};
Camera.prototype.kind = "camera";
Camera.prototype.read = function(delta) {
    return this.count + delta;
};

let front = new Camera("front");
let side = new Camera("side");
front.count = 41;

let keys = "";
for (let key in front) {
    keys = keys + key + ";";
}

let Replace = function Replace() {
    this.value = 1;
    return { value: 42 };
};
let Keep = function Keep() {
    this.value = 42;
    return 7;
};

let replaced = new Replace();
let kept = new Keep();

print(front.name, side.name, front.kind, front.read(1), side.read(2));
print("read" in front, "kind" in front, front.__proto__ === Camera.prototype);
print(keys);
print(replaced.value, kept.value);
