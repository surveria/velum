let total = 0;
let Camera = function Camera(value) {
    this.value = value;
};
Camera.prototype.bump = function(delta) {
    this.value += delta;
    return this.value;
};
Camera.prototype.read = function() {
    return this.value;
};

for (let index = 0; index < 1200000; index++) {
    let camera = new Camera(index);
    total += camera.bump(1);
    total += camera.read();
    if ("bump" in camera) {
        total += 1;
    }
}

total
