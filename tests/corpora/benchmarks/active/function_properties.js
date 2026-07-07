let total = 0;
let fn = function camera(left, right, extra) {
    return left + right + extra;
};

for (let index = 0; index < 8192; index++) {
    total += fn.length;
    if (fn.name === "camera") {
        total += 1;
    }
    if ("length" in fn) {
        total += 1;
    }
    if (fn.missing === undefined) {
        total += 1;
    }
}

total
