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

print(total);

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

print(selected);

let fallback = 0;
switch ("missing") {
    case "camera":
        fallback = 1;
        break;
    default:
        fallback = 20;
    case "lens":
        fallback = fallback + 22;
        break;
}

print(fallback);

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

print(loopTotal);

switch (0) {
    case 1:
        var hoisted = 42;
}

print(hoisted);
