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

if (front.name !== "front" || side.name !== "side") {
    throw new Test262Error("constructor did not initialize instances");
}
if (front.kind !== "camera" || front.read(1) !== 42 || side.read(2) !== 42) {
    throw new Test262Error("constructor prototype lookup was unexpected");
}
if (!("read" in front) || !("kind" in front)) {
    throw new Test262Error("constructor prototype membership was unexpected");
}
if (front.__proto__ !== Camera.prototype) {
    throw new Test262Error("constructed object prototype was unexpected");
}
if (keys !== "name;count;kind;read;") {
    throw new Test262Error("constructed object enumeration was unexpected");
}

let Replace = function Replace() {
    this.value = 1;
    return { value: 42 };
};
let Keep = function Keep() {
    this.value = 42;
    return 7;
};
if ((new Replace()).value !== 42 || (new Keep()).value !== 42) {
    throw new Test262Error("constructor return handling was unexpected");
}

42
