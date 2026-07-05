let total = 0;
let prototype = Object.prototype;

for (let index = 0; index < 128; index++) {
    let plain = {};
    let created = Object();
    let constructed = new Object();

    if (plain.constructor === Object) {
        total += 1;
    }
    if (created.__proto__ === prototype) {
        total += 1;
    }
    if (constructed.__proto__ === prototype) {
        total += 1;
    }
    if (Object(plain) === plain) {
        total += 1;
    }
    if (new Object(plain) === plain) {
        total += 1;
    }
}

total
