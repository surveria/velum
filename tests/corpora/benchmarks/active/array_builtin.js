let total = 0;
let prototype = Array.prototype;

for (let index = 0; index < 4096; index++) {
    let literal = [index, index + 1];
    let created = Array();
    let constructed = new Array();
    let withElements = Array(index, index + 1);

    if (literal.__proto__ === prototype) {
        total += 1;
    }
    if (literal.constructor === Array) {
        total += 1;
    }
    if (created.length === 0) {
        total += 1;
    }
    if (constructed.__proto__ === prototype) {
        total += 1;
    }
    if (withElements.length === 2) {
        total += 1;
    }
    if (withElements[1] === index + 1) {
        total += 1;
    }
}

total
