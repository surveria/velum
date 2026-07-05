let camera = { value: 40, name: "front" };
camera.read = function(delta) {
    return this.value + delta;
};
camera.write = function(value) {
    this.value = value;
    return this.read(0);
};

let first = camera.read(2);
let second = camera["write"](42);
let parenthesized = (camera.read)(0);
let keywordProperty = { this: "keyword" }.this;

print(first, second, parenthesized, camera.value, keywordProperty);
