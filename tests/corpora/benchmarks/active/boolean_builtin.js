let total = 0;

for (let index = 0; index < 4096; index++) {
    if (Boolean(index)) {
        total += 1;
    }
    if (!Boolean(0)) {
        total += 1;
    }
    if (Boolean("camera")) {
        total += 1;
    }
    if (!Boolean("")) {
        total += 1;
    }

    let boxed = new Boolean(false);
    if (boxed.__proto__ === Boolean.prototype) {
        total += 1;
    }
    if (boxed.constructor === Boolean) {
        total += 1;
    }
    if (Boolean(boxed)) {
        total += 1;
    }
}

if (Boolean.prototype.constructor === Boolean) {
    total += 1;
}

total
