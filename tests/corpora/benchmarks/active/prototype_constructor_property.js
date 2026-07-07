let total = 0;
let Camera = function Camera(value) {
    this.value = value;
};

for (let index = 0; index < 4096; index++) {
    let keys = "";
    for (let key in Camera.prototype) {
        keys = keys + key + ";";
    }
    if (keys === "") {
        total += 1;
    }
    if ("constructor" in Camera.prototype) {
        total += 1;
    }
    if (Camera.prototype.constructor === Camera) {
        total += 1;
    }
    let camera = new Camera(index);
    if (camera.__proto__.constructor === Camera) {
        total += 1;
    }
}

total
