let total = 0;
switch ("camera") {
    case "sensor":
        total = 1;
        break;
    case "camera":
        total = total + 20;
    case "lens":
        total = total + 22;
        break;
    default:
        total = 0;
}

let selected = "none";
switch (2) {
    case 1:
        selected = "one";
        break;
    default:
        selected = "default";
        break;
    case 2:
        selected = "two";
        break;
}

let loopTotal = 0;
for (let index = 0; index < 5; index = index + 1) {
    switch (index) {
        case 1:
            continue;
        case 3:
            break;
        default:
            loopTotal = loopTotal + index;
    }
    loopTotal = loopTotal + 10;
}

print(total, selected, loopTotal);
total
