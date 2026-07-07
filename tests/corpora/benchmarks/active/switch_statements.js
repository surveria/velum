let values = [1, 2, 3, 4];
let total = 0;

for (let index = 0; index < 98304; index = index + 1) {
    switch (index & 3) {
        case 0:
            total = total + values[0];
            break;
        case 1:
            total = total + values[1];
            break;
        case 2:
            total = total + values[2];
            break;
        default:
            total = total + values[3];
    }
}

total;
