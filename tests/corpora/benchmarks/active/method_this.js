let total = 0;
let camera = { value: 1 };
camera.bump = function(delta) {
    this.value += delta;
    return this.value;
};
camera.read = function() {
    return this.value;
};

for (let index = 0; index < 8192; index++) {
    total += camera.bump(1);
    total += camera["read"]();
}

total
